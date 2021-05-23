# dgraph-admin

```console
$ dgraph-admin help
Usage: dgraph-admin [--url <url>] [--auth <auth>] <command> [<args>]

dgraph-admin is a simple tool for managing dgraph.

Options:
  --url             dgraph url
  --auth            auth header to include with the request

Commands:
  update-schema     add or modify schema
  get-schema        get the current schema
  drop-all          drop all data and schema
  drop-data         drop all data only (keep schema)
  get-health        get status of nodes
```

## installation

you will need [rust and cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

```console
$ git clone https://github.com/blukai/dgraph-admin.git
$ cd dgraph-admin
$ cargo build --path .
```

## usage

dgraph-admin can be used to manage dgraph that is running locally as well as in dgraph cloud

### locally

if dgraph is available on default url (localhost:8080) and token is not set:

```console
$ dgraph-admin get-health
```

if token was provided in dgraph's `--security` flag (see [dgraph cli ref](https://dgraph.io/docs/deploy/cli-command-reference/#dgraph-core-commands)):

```console
$ dgraph-admin --auth X-Dgraph-AuthToken:token get-health
```

### with dgraph cloud

to be able to use it with dgraph cloud you will need to create an admin api key (see [authentication](https://dgraph.io/docs/cloud/admin/authentication/))

```console
$ dgraph-admin --url https://something.cloud.dgraph.io --auth Dg-Auth:key get-health
```

## a piece of motivation

during development (other project) i found an often need to modify schema or drop all data.
while it is possible without this tool, it is not as convenient, easy and fast.

also i wanted to write something useful in rust xd.
