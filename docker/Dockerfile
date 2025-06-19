
#XXX Build fail on alpine 
# See https://github.com/tkaitchuck/constrandom/issues/38

#FROM rust:1-alpine3.22 AS builder
FROM rust:1-slim-bookworm AS builder

RUN --mount=type=bind,target=/src,rw \
    cd /src && cargo build --release --target-dir /build

#FROM alpine:3.22
FROM debian:bookworm-slim as server

LABEL org.opencontainers.image.vendor="3liz"
LABEL org.opencontainers.image.title="PG-event-Server"
LABEL org.opencontainers.image.description="HTTP Postgres event server"
LABEL org.opencontainers.image.authors="David Marteau <david.marteau@3liz.com>"
LABEL org.opencontainers.image.ref.name="3liz/pg-event-server" 

COPY --from=builder /build/release/pg-event-server /usr/local/bin/

# Default configuration
RUN adduser --system server
RUN mkdir /etc/pg-event-server.d
COPY docker/default-config.toml /etc/pg-event-server.toml 

ENV PATH=/usr/local/bin/:$PATH

EXPOSE 4001

USER server

ENTRYPOINT ["pg-event-server"]
CMD ["--conf", "/etc/pg-event-server.toml"]
