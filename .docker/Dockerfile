# syntax=docker/dockerfile:1
ARG REGISTRY_PREFIX=''

FROM ${REGISTRY_PREFIX}rust:1-slim-bookworm AS builder

RUN --mount=type=bind,target=/src,rw \
    --mount=type=cache,target=/.cargo \
    cd /src && \
    cp pg-client-config/fixtures/passfile.conf.in pg-client-config/fixtures/passfile.conf && \
    chmod 0600 pg-client-config/fixtures/passfile.conf && \
    export CARGO_HOME=/.cargo && \
    cargo test  --release --target-dir /build && \
    cargo build --release --target-dir /build 

FROM debian:bookworm-slim

LABEL org.opencontainers.image.vendor="3liz"
LABEL org.opencontainers.image.title="PG-event-Server"
LABEL org.opencontainers.image.description="HTTP Postgres event server"
LABEL org.opencontainers.image.authors="David Marteau <david.marteau@3liz.com>"
LABEL org.opencontainers.image.ref.name="3liz/pg-event-server" 

COPY --from=builder /build/release/pg-event-server /usr/local/bin/

# Default configuration
RUN adduser --system server
RUN mkdir /etc/pg-event-server.d
COPY .docker/default-config.toml /etc/pg-event-server.toml 

ENV PATH=/usr/local/bin/:$PATH

EXPOSE 4001

USER server

ENTRYPOINT ["pg-event-server"]
CMD ["--conf", "/etc/pg-event-server.toml"]
