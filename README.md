# HTTP Postgres event server

Relay Postgres event to HTTP SSE channels.

1. Configure published Postgres channels 
2. Client subscribe to SSE channels 
3. Relay event to SSE subscribers.

## Features

* Multiple databases
* TLS connection support for remote postgres server
* SSL server connection

## Operational 

At startup, the server open LISTEN sessions to configured remote postgres 
server(s).

Clients connect to server with HTTP connection opened in SSE mode using predefined
subscription url (see below). 

All events received from remote database are
broadcasted to connected clients.

## Running the server 

```
Usage: pg-event-server [OPTIONS] --conf <CONF>

Options:
      --conf <CONF>  Path to configuration file
  -v, --verbose...   Increase verbosity
      --check        Check configuration only
  -h, --help         Print help
  -V, --version      Print version
```

## Configuration

Configuration is in ["toml"](https://github.com/toml-lang/toml/wiki) format.

### `[Server]` settings

* `title` - Server title that will appear in the `Server` header; optional.
* `listen` - Interface to listen to as `interface:port` string; required.
* `ssl_enabled` - Enable SSL http connections (default to `false`)
* `ssl_key_file` - Path to SSL key  file (absolute or relative to config file)
* `ssl_key_file` - Path to SSL cert file (absolute or relative to config file)

### `[postgres_tls]` 

* `tls_ca_file` - CA cert file for self-signed certificats
* `tls_client_auth_key` - Path to key file that contains the client authentification 
   key to present to remote server.
* `tls_client_auth_cert` - Path to cert file containing the client authentification 
   cert to present to remote server

### `[[channel]]`

Postgres channel configuration.

A channel configuration has the following format: 

```
[[channel]]
id = "my/test"
allowed_events = ["foo", "bar", "baz"]
connection_string = "service=local"
```

There is no limit in the number of channels that can de declared in the
configuration.

#### Channel parameters

* `id` - The identification of the channel: it should be formatted as a valid path.
         The `id` will be used as the subscription path for clients. 
* `allowed_events` - Optional - The list of events that will be forwarded 
   to the client listening to that channel. If not present, all events will be forwarded.
* `connection_string` - The postgres connection string. The format of the connection 
   follow the forme described [here](https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html).
   If the connection string *starts* with "service=" then the corresponding service
   will be searched using the same rules as used for service in [libpq](https://docs.postgresql.fr/10/libpq-pgservice.html)

Furthemore the following environment variables are supported:

* `PGSERVICE` - Name of the postgres service used for connection params.
* `PGSYSCONFDIR` - Location of the service files.
* `PGSERVICEFILE` - Name of the service file.
* `PGHOST` - behaves the same as the `host` connection parameter.
* `PGPORT` - behaves the same as the `port` connection parameter.
* `PGDATABASE` - behaves the same as the `dbname` connection parameter.
* `PGUSER` - behaves the same as the user connection parameter.
* `PGOPTIONS` - behaves the same as the `options` parameter.
* `PGAPPNAME` - behaves the same as the `application_name` connection parameter.
* `PGCONNECT_TIMEOUT` - behaves the same as the `connect_timeout` connection parameter.
* `PGPASSFILE` - Specifies the name of the file used to store password.

### Postgres channel configurations in separate files

Multiple channel configurations will be searched in the `<config_name>.d` directory located
at the same lever as the configuration file `<config_name>.toml`.

All files ending by `.toml` will be loaded for channel configuration.

### Subscription url

```
http://{host:port}/event/subscribe/{channel_path}"
```

Where `{channel_path}` is any `id` configured to a Postgres event channel.

## Connection to databases

The server allow to connecting to multiple database defined in the channel. 
There will be only one connection per user, host and database. 

These connections are open at server startup and no more connection 
will be opened during the running time of the server.

## TLS support for database connection

TLS connection are available from [`rustls`](https://docs.rs/rustls/latest/rustls/) 
implementation, 

### Using self-signed CA certificates

By default, platform certficates are used for checking the validity of the server certificate.

Custe CA certficates may be used with the  `tls_ca_file` option in `[the postgres_tls]` section.
In this case platform certificates are not used.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
