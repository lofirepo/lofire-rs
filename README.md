# LoFiRe Rust implementation

## Design & specification

[LoFiRe: Local-First Repositories for Asynchronous Collaboration over Community Overlay Networks](https://p2pcollab.net/design/lofire)

## Development

### Cargo

Build, test, and generate documentation:

```
cargo build
cargo test --all --verbose -- --nocapture
cargo doc
```

Run the `lofire-node` daemon and `lofire-demo`:

```
cargo run --bin lofire-node
cargo run --bin lofire-demo
```

### Nix

Install the [Nix package manager](https://nixos.org/download.html)
and [Nix Flakes](https://nixos.wiki/wiki/Flakes)

#### Get a development shell

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
