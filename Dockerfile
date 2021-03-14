FROM rust:latest as build

COPY . .

RUN cargo build --release

FROM ubuntu:latest

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get -y update && \
    apt-get -y upgrade  && \
    apt -y install ca-certificates libssl-dev libpq-dev

COPY --from=build /target/release/groot /usr/local/bin
CMD ["/usr/local/bin/groot", "--serve"]
