# syntax=docker/dockerfile:1.7-labs
FROM rust:1.83-bullseye AS builder

ARG BUILD_ARGS=""

WORKDIR /builder

COPY --parents Cargo.lock **/Cargo.toml ./
RUN find . -type f |sort # && false
RUN \
    mkdir backend/src && echo "fn main() {}" > backend/src/main.rs; \
    mkdir skjera_api/src && echo "fn main() {}" > skjera_api/src/main.rs;
RUN cargo fetch
RUN cargo build $BUILD_ARGS

COPY . .
RUN cargo build $BUILD_ARGS

FROM debian:bullseye AS app

ARG TARGET_DIR="debug"
ARG DEBIAN_FRONTEND=noninteractive

WORKDIR /app

RUN apt-get update && apt-get install --no-install-recommends --yes ca-certificates
COPY --from=builder /builder/target/$TARGET_DIR/skjera /
COPY --from=builder /builder/backend/assets ./assets

CMD ["/skjera"]
EXPOSE 8080
