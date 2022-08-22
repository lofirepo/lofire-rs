{
  description = "LoFiRe";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-22.05";
  inputs.utils.url = "github:numtide/flake-utils";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs = {
    self,
    nixpkgs,
    utils,
    rust-overlay,
  }:
    utils.lib.eachDefaultSystem (system: let
      overlays = [
        (import rust-overlay)
      ];
      pkgs = import nixpkgs rec {
        inherit system overlays;
      };
      rust = pkgs.rust-bin.stable."1.62.0".default.override {
        extensions = ["rust-src"];
      };
      buildRustPackage =
        (pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        })
        .buildRustPackage;
      myNativeBuildInputs = with pkgs; [
      ];
      myBuildInputs = with pkgs; [
      ];
      myBuildRustPackage = attrs:
        buildRustPackage ({
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = myNativeBuildInputs;
            buildInputs = myBuildInputs;
            #RUST_BACKTRACE=1;
            #RUST_LOG="trace";
          }
          // attrs);
    in rec {
      packages = rec {
        lofire-repo = myBuildRustPackage rec {
          pname = "lofire-repo";
          buildAndTestSubdir = "./lofire-repo";
        };
        default = lofire-repo;
      };
      defaultPackage = packages.default; # compat
    });
}
