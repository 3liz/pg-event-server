ARG REGISTRY_PREFIX=''

FROM ${REGISTRY_PREFIX}rust:1-slim-bullseye as builder

WORKDIR /usr/src/
COPY . .
RUN cd ./pg-event-server/ && cargo install --path .

FROM ${REGISTRY_PREFIX}debian:bullseye-slim
RUN apt-get update && apt-get install && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/pg-event-server /usr/local/bin/pg-event-server
COPY .docker/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]

