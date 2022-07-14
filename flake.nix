{
  description = "LoFiRe";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-22.05";
  inputs.utils.url = "github:numtide/flake-utils";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs = { self, nixpkgs, utils, rust-overlay }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          (import rust-overlay)
        ];
        pkgs = import nixpkgs rec {
          inherit system overlays;
        };
        rust = pkgs.rust-bin.stable."1.62.0".default.override {
          extensions = [ "rust-src" ];
        };
        buildRustPackage = (pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        }).buildRustPackage;
      in {
        packages = rec {
          default = lofire;
          lofire = buildRustPackage rec {
            pname = "lofire";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
          lofire-repo = buildRustPackage rec {
            pname = "lofire-repo";
            version = "0.1.0";
            src = ./.;
            buildAndTestSubdir = "./lofire-repo";
            cargoLock.lockFile = ./Cargo.lock;
          };
        };
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rust
          ];
        };
      });
}
