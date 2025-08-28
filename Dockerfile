FROM rust:1.82-slim AS build

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release

# Use Debian for glibc compatibility
FROM debian:12-slim

RUN apt-get update && apt-get install -y \
    libpq5 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd -g 1001 groot && \
    useradd -r -u 1001 -g groot groot

# Copy binary from builder stage
COPY --from=build /target/release/groot /groot
RUN chown groot:groot /groot

# Create content directories
RUN mkdir -p /content/collections /content/roles && \
    chown -R groot:groot /content

USER groot
EXPOSE 3030

CMD ["/groot"]
