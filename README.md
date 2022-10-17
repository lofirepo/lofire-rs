# LoFiRe Rust implementation

## Design & specification

[LoFiRe: Local-First Repositories for Asynchronous Collaboration over Community Overlay Networks](https://p2pcollab.net/design/lofire)

## API documentation

[LoFiRe Rust API Documentation](https://p2pcollab.github.io/lofire-rs/doc/)

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
