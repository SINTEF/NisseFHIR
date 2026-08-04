FROM rust:slim AS build

WORKDIR /srv/app

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first and cache dependency fetch
COPY Cargo.toml Cargo.lock ./

RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Copy schema file needed by include_str!("../fhir.schema.json") at compile time
COPY fhir.schema.json ./fhir.schema.json

# Copy actual source and migrations
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

LABEL org.opencontainers.image.source="https://github.com/SINTEF/NisseFHIR"
LABEL org.opencontainers.image.description="NisseFHIR – Lightweight FHIR R6 Server"
LABEL org.opencontainers.image.licenses="WTFPL AND CeCILL-B"
LABEL org.opencontainers.image.version="0.1.4"

WORKDIR /srv/app

COPY --from=build /srv/app/target/release/fhir_server /usr/local/bin/fhir_server

ENV BIND_ADDR=0.0.0.0:8080 \
    METRICS_ENABLED=true \
    METRICS_BIND_ADDR=0.0.0.0:9090

EXPOSE 8080 9090

ENTRYPOINT ["fhir_server"]
