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
nix flake build
```

### Build a specific package
```
nix flake build '.#lofire-repo'
```
