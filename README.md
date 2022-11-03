# LoFiRe Rust implementation

## Website

[LoFiRe](https://lofi.re)

## Design & specification

[LoFiRe: Local-First Repositories for Asynchronous Collaboration over Community Overlay Networks](https://lofi.re/design/lofire)

## API documentation

[LoFiRe Rust API Documentation](https://p2pcollab.github.io/lofire-rs/doc/)

## Overview

The following components are implemented so far:

- lofire: library that allows access to the repository, branches, commits, objects, blocks, and contains a hash map backed store implementation.
- lofire-store-lmdb: encrypted LMDB store implementation
- lofire-net: library that provides network message types
- lofire-broker: library that implements the broker server and client protocol with async, this allows running them via arbitrary network transports or in-process without networking
- lofire-node: daemon that runs a websocket server and the broker protocol over it
- lofire-demo: an application to demonstrate the usage and functionality that connects to the node and sends messages to it

For examples on using the libraries, see the test cases and the demo application.
To run the demo, first run lofire-node, then lofire-demo (see below).

## Development

### Cargo

#### Build

Build all packages:

```
cargo build
```

#### Test

Test all:

```
cargo test --all --verbose -- --nocapture
```

Test a single module:

```
cargo test --package lofire --lib -- branch::test --nocapture
```

#### Documentation

Generate documentation for all packages without their dependencies:

```
cargo doc --no-deps
```

The generated documentation can be found in `target/doc/<crate-name>`.

#### Run

Build & run executables:

```
cargo run --bin lofire-node
cargo run --bin lofire-demo
```

### Nix

Install the [Nix package manager](https://nixos.org/download.html)
and [Nix Flakes](https://nixos.wiki/wiki/Flakes)

#### Development shell

Get a development shell with all dependencies available:

```
nix develop
cargo build
...
```

#### Build a package

Build the default package (`.#lofire-node`):

```
nix build
```

Bulid a specific package:

```
nix build '.#lofire'
```

#### Run

Run the default executable (`.#lofire-node`):

```
nix run
```

Run executables:

```
nix run '.#lofire-node'
nix run '.#lofire-demo'
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly stated otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
