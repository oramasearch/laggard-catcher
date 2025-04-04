use anyhow::{Context, Result};
use futures::StreamExt;
use rabbitmq_stream_client::{
    Environment,
    types::{Message, OffsetSpecification},
};

pub struct Bumper {
    environment: Environment,
    send_stream_name: String,
    receive_stream_name: String,
    location_application_property_name: String,
    location_application_property_value: String,
}

impl Bumper {
    pub fn new(
        environment: Environment,
        send_stream_name: String,
        receive_stream_name: String,
        location_application_property_name: String,
        location_application_property_value: String,
    ) -> Self {
        Bumper {
            environment,
            send_stream_name,
            receive_stream_name,
            location_application_property_name,
            location_application_property_value,
        }
    }

    pub async fn run(self) -> Result<()> {
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

        while let Some(Ok(delivery)) = consumer.next().await {
            println!("Received message");
            let message = delivery.message();

            let message_builder = Message::builder();
            let message_builder = if let Some(data) = message.data() {
                message_builder.body(data)
            } else {
                message_builder
            };
            let message_builder = message_builder
                .application_properties()
                .insert(
                    self.location_application_property_name.clone(),
                    self.location_application_property_value.clone(),
                )
                .message_builder();
            let message = message_builder.build();

            producer
                .send(message, |_| async {})
                .await
                .context("Failed to send message")?;
            println!("Sent message");
        }

        Ok(())
    }
}
