# Laggard Catcher
A tool to catch laggard messages from RabbitMQ streams

## Installation

```bash
cargo build --release
```

## Usage

From one machine:
```bash
laggard-catcher \
    --send-stream-name producer-to-consumer \
    --receive-stream-name consumer-to-producer \
    --location-application-property-name othermachine-name \
    catcher --period 5s \
        --http-port 8888 \
        --http-host localhost
```

From another machine:
```bash
laggard-catcher \
    --receive-stream-name producer-to-consumer \
    --send-stream-name consumer-to-producer \
    --location-application-property-name reader-name \
    bumper --location-application-property-value machine-name
```

For more information, run:
```bash
laggard-catcher --help
```

