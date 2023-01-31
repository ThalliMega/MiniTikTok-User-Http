This is *just* a homework project using [![Rust]](https://www.rust-lang.org "Rust").

# Ports

Server listens on `0.0.0.0:14514` and `[::]:14514`.

# Environment Variables

## Runtime env vars

### BOLT_URL

The address of the graph database.

### BOLT_DOMAIN

This env var is optional.
If set, TLS negotiation will be attempted.

### BOLT_USERNAME

The username of the graph database.

### BOLT_PASSWORD

The password of the graph database.

### AUTH_URL

The url of the Auth Grpc service.

### USER_URL

The url of the User Grpc service.

### POSTGRES_URL

Check the [documention](https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html) for details.

### RUST_LOG

Check the [documention](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for details.

### RUST_LOG_STYLE

Check the [documention](https://docs.rs/env_logger/latest/env_logger/#disabling-colors) for details.

## Buildtime env vars

When build the image, specify build args with [--build-args](https://docs.docker.com/engine/reference/commandline/build/#-set-build-time-variables---build-arg).

### REPLACE_ALPINE

This value will be passed to [sed](https://manpages.org/sed) as a script when modifying [/etc/apk/repositories](https://man.archlinux.org/man/community/apk-tools/apk-repositories.5.en).

[Rust]: https://img.shields.io/badge/Rust-ffffff?style=for-the-badge&labelColor=ffffff&logoColor=000000&logo=rust