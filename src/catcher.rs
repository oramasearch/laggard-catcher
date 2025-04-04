use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::Router;
use futures::StreamExt;
use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use rabbitmq_stream_client::{
    Environment,
    types::{Message, OffsetSpecification, SimpleValue},
};

pub struct Catcher {
    environment: Environment,
    send_stream_name: String,
    receive_stream_name: String,
    location_application_property_name: String,
    period: Duration,
    http_host: String,
    http_port: u16,
}

impl Catcher {
    pub fn new(
        environment: Environment,
        send_stream_name: String,
        receive_stream_name: String,
        location_application_property_name: String,
        period: Duration,
        http_host: String,
        http_port: u16,
    ) -> Self {
        Catcher {
            environment,
            send_stream_name,
            receive_stream_name,
            location_application_property_name,
            period,
            http_host,
            http_port,
        }
    }

    pub async fn run(self) -> Result<()> {
        println!("Starting catcher");
        let mut consumer = self
            .environment
            .consumer()
            .offset(OffsetSpecification::Next)
            .build(&self.receive_stream_name)
            .await
            .context("Failed to create consumer")?;
        let producer = self
            .environment
            .producer()
            .build(&self.send_stream_name)
            .await
            .context("Failed to create producer")?;

        let period = self.period;
        tokio::spawn(async move {
            println!("Starting producer with period: {:?}", period);
            let mut interval = tokio::time::interval(period);
            loop {
                interval.tick().await;

                let millisec = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis();
                let bytes = millisec.to_be_bytes();

                let message = Message::builder().body(bytes).build();

                producer
                    .send(message, |_| async {})
                    .await
                    .context("Failed to send message")
                    .unwrap();
                println!("Message sent");
                counter!("laggard_catcher.message_sent").increment(1);
            }
        });

        let prometheus_handle = PrometheusBuilder::new()
            .set_quantiles(&[0.0, 0.5, 0.95, 1.0])
            .context("failed to set quantiles")?
            .install_recorder()
            .context("failed to install recorder")?;

        tokio::spawn(async move {
            let router = Router::new().route(
                "/metrics",
                axum::routing::get(move || async move { prometheus_handle.render() }),
            );
            let addr = format!("{}:{}", self.http_host, self.http_port);
            println!("Starting HTTP server on {:?}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

            axum::serve(listener, router)
                .await
                .expect("Failed to start server");
        });

        println!("Waiting for messages...");
        let location_application_property_name = &self.location_application_property_name;
        while let Some(Ok(delivery)) = consumer.next().await {
            println!("Received message");
            let message = delivery.message();

            let app_props_value = message
                .application_properties()
                .and_then(|p| p.get(location_application_property_name))
                .and_then(|p| match p {
                    SimpleValue::String(s) => Some(s),
                    _ => None,
                });
            let app_props_value = match app_props_value {
                Some(value) => value,
                None => {
                    println!("No application property value");
                    continue;
                }
            };

            let data = match message.data() {
                Some(data) => data,
                None => {
                    println!("No data in message");
                    continue;
                }
            };
            let start: u128 = match data[0..16].try_into() {
                Ok(data) => u128::from_be_bytes(data),
                Err(_) => {
                    println!("Failed to convert data to u128");
                    continue;
                }
            };
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();

            let diff = now - start;
            let diff_in_sec = diff as f64 / 1000.0;
            histogram!(
                "laggard_catcher.delay",
                "value" => app_props_value.to_string(),
            )
            .record(diff_in_sec);
            println!("Delay: {} sec", diff_in_sec);
        }
        Ok(())
    }
}
