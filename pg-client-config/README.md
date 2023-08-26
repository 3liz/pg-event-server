# Tokio-postgres support for service file and environment variables.

Connection string parsing with support for service file
and a subset of psql environment variables.

*Note*: tokio-postgres 0.7.9 [introduced a change](https://github.com/3liz/pg-event-server/issues/1)
preventing `PGUSER` and service configuration to set connection user. 
The [release of tokio-postgres 0.7.10](https://github.com/sfackler/rust-postgres/blob/master/tokio-postgres/CHANGELOG.md#v0710---2023-08-25)
fix this issue.

## Environment variables

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

## Passfile support 

Passfile is actually supported only on linux platform

## Example

```
use pg_client_config::load_config;

let config = load_config(Some("service=myservice")).unwrap();
println!("{config:#?}");
```

## Precedence rules

* Environment variables are always evaluated with the least precedence.
* Parameters passed in the connection string always take precedence.

## See also

* [Pg service file](https://www.postgresql.org/docs/current/libpq-pgservice.html)
* [Pg pass file](https://www.postgresql.org/docs/current/libpq-pgpass.html)
