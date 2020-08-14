FROM rust:1.45 as builder
WORKDIR /usr/src/tshock-discord-monitor
COPY . .
RUN cargo install --path .

FROM ubuntu:latest
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=builder /usr/local/cargo/bin/tshock_discord_monitor /usr/local/bin/tshock_discord_monitor
WORKDIR /
CMD ["tshock_discord_monitor"]