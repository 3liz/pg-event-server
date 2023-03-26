# HTTP Postgres event server

Relay Postgres event to HTTP SSE channels.

1. Configure published Postgres channels 
2. Client subscribe to SSE channels 
3. Relay event to SSE subscribers.

## Operational 

 

## Listening to channels

### Subscription url

```
http://{host:port}/event/subscribe/{channel_path}"
```

Where `{channel_path}` is any path configured to a Postgres event channel.

## Connection to databases

The server allow to connecting to multiple database defined in the channel. 
There will be only one connection per user and database. 

These connections are open at server startup and no more connection 
will be opened during the running time of the server.
