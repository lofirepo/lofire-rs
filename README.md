# LoFiRe Rust implementation

## Design & specification

[LoFiRe: Local-First Repositories for Asynchronous Collaboration over Community Overlay Networks](https://p2pcollab.net/design/lofire)

## Development

### Requirements

Install the [Nix package manager](https://nixos.org/download.html)
and [Nix Flakes](https://nixos.wiki/wiki/Flakes)

### Get a development shell
```
nix develop
cargo build
```

### Build the default package
```
nix build
```

### Build a specific package
```
nix build '.#lofire-repo'
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly stated otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
