use std::{ops::Deref, str::FromStr, time::Duration};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use laggard_catcher::{bumper::Bumper, catcher::Catcher};
use rabbitmq_stream_client::{Environment, error::StreamCreateError, types::ResponseCode};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    /// Sets a custom config file
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    user: Option<String>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    v_host: Option<String>,
    #[arg(long)]
    load_balancer_mode: Option<bool>,
    #[arg(long)]
    client_provided_name: Option<String>,

    #[arg(long)]
    send_stream_name: String,
    #[arg(long)]
    receive_stream_name: String,
    #[arg(long)]
    location_application_property_name: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Catcher {
        #[arg(long)]
        period: DurationFromClap,
        #[arg(long)]
        http_port: u16,
        #[arg(long)]
        http_host: String,
    },
    Bumper {
        #[arg(long)]
        location_application_property_value: String,
    },
}

#[derive(Clone)]
struct DurationFromClap(Duration);

impl FromStr for DurationFromClap {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration = duration_str::parse(s)?;
        Ok(DurationFromClap(duration))
    }
}
impl Deref for DurationFromClap {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let environment_builder = Environment::builder();
    let environment_builder = if let Some(host) = cli.host {
        environment_builder.host(&host)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(port) = cli.port {
        environment_builder.port(port)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(user) = cli.user {
        environment_builder.username(&user)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(password) = cli.password {
        environment_builder.password(&password)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(v_host) = cli.v_host {
        environment_builder.virtual_host(&v_host)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(load_balancer_mode) = cli.load_balancer_mode {
        environment_builder.load_balancer_mode(load_balancer_mode)
    } else {
        environment_builder
    };
    let environment_builder = if let Some(client_provided_name) = cli.client_provided_name {
        environment_builder.client_provided_name(&client_provided_name)
    } else {
        environment_builder
    };
    let environment = environment_builder
        .build()
        .await
        .context("Cannot create environment")?;

    ensure_stream_is_created(&environment, &cli.send_stream_name)
        .await
        .context("Cannot create 'send_stream_name' stream")?;
    ensure_stream_is_created(&environment, &cli.receive_stream_name)
        .await
        .context("Cannot create 'receive_stream_name' stream")?;

    match cli.command {
        Commands::Catcher {
            period,
            http_host,
            http_port,
        } => {
            let catcher = Catcher::new(
                environment,
                cli.send_stream_name,
                cli.receive_stream_name,
                cli.location_application_property_name,
                period.0,
                http_host,
                http_port,
            );
            catcher.run().await.context("Failed to run catcher")?;
        }
        Commands::Bumper {
            location_application_property_value,
        } => {
            let bumper = Bumper::new(
                environment,
                cli.send_stream_name,
                cli.receive_stream_name,
                cli.location_application_property_name,
                location_application_property_value,
            );
            bumper.run().await.context("Failed to run bumper")?;
        }
    }

    Ok(())
}

async fn ensure_stream_is_created(environment: &Environment, stream_name: &str) -> Result<()> {
    let stream_creator = environment.stream_creator();
    match stream_creator.create(stream_name).await {
        Ok(_) => {
            println!("Stream {} created", stream_name);
        }
        Err(StreamCreateError::Create { status, .. })
            if status == ResponseCode::StreamAlreadyExists =>
        {
            println!("Stream {} already exists", stream_name);
        }
        Err(e) => {
            bail!("Failed to create stream {}: {:?}", stream_name, e);
        }
    }
    Ok(())
}
