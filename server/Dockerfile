FROM rust:slim AS build

WORKDIR /srv/app

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first and cache dependency fetch
COPY server/Cargo.toml server/Cargo.lock ./server/

RUN cd server \
    && mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Copy schema file needed by include_str!("../../fhir.schema.json") at compile time
COPY fhir.schema.json ./fhir.schema.json

# Copy actual source and migrations
COPY server/src ./server/src
COPY server/migrations ./server/migrations

RUN cd server && cargo build --release

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

WORKDIR /srv/app

COPY --from=build /srv/app/server/target/release/fhir_server /usr/local/bin/fhir_server

ENV BIND_ADDR=0.0.0.0:8080

EXPOSE 8080

ENTRYPOINT ["fhir_server"]
