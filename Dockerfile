# syntax=docker/dockerfile:1
# adapted from https://docs.docker.com/language/rust/develop/#get-and-run-the-sample-application

ARG RUST_VERSION=1.77.2
ARG APP_NAME=mintpool
FROM rust:${RUST_VERSION}-slim-bullseye AS build
ARG APP_NAME
WORKDIR /app

ENV DATABASE_URL=sqlite:/app/dev.db

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev

RUN --mount=type=bind,source=justfile,target=justfile \
    --mount=type=bind,source=migrations,target=migrations \
    cargo install just && just ci

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=.git,target=.git \
    --mount=type=bind,source=data,target=data \
    --mount=type=bind,source=contracts,target=contracts \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=bind,source=migrations,target=/app/migrations \
    <<EOF
set -e
cargo build --locked --release
cp ./target/release/$APP_NAME /bin/server
EOF


FROM debian:bullseye-slim AS final

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

COPY --from=build /bin/server /bin/
ADD ./migrations /migrations

EXPOSE 8000

CMD ["/bin/server"]