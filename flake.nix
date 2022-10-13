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
        pkgconfig
      ];
      myBuildInputs = with pkgs;
        [
          openssl
        ]
        ++ lib.optionals stdenv.isDarwin
        (with darwin.apple_sdk.frameworks; [
          Security
        ]);
      myBuildRustPackage = attrs:
        buildRustPackage ({
            version = "0.1.0";
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "lmdb-crypto-rs-0.14.0" = "sha256-Js5pmytq4N52yClhzeps5qHnTTrRGRUMX16ss8FWgwo=";
                "rkv-0.18.0" = "sha256-TnU+I6DINegLlzKPZ1Pet8Kr5x/WqEo2kwxQU3q7G34=";
              };
            };
            nativeBuildInputs = myNativeBuildInputs;
            buildInputs = myBuildInputs;
            RUST_BACKTRACE = 1;
          }
          // attrs);
    in rec {
      packages = rec {
        lofire = myBuildRustPackage rec {
          pname = "lofire";
          buildAndTestSubdir = "./lofire";
        };
        lofire-broker = myBuildRustPackage rec {
          pname = "lofire-broker";
          buildAndTestSubdir = "./lofire-broker";
        };
        lofire-p2p = myBuildRustPackage rec {
          pname = "lofire-p2p";
          buildAndTestSubdir = "./lofire-p2p";
        };
        lofire-store-lmdb = myBuildRustPackage rec {
          pname = "lofire-store-lmdb";
          buildAndTestSubdir = "./lofire-store-lmdb";
        };
        lofire-node = myBuildRustPackage rec {
          pname = "lofire-node";
          buildAndTestSubdir = "./lofire-node";
        };
        lofire-demo = myBuildRustPackage rec {
          pname = "lofire-demo";
          buildAndTestSubdir = "./lofire-demo";
        };
        default = lofire-node;
      };

      apps = rec {
        lofire-node = utils.lib.mkApp {
          drv = packages.lofire-node;
          exePath = "/bin/lofire-node";
        };
        lofire-demo = utils.lib.mkApp {
          drv = packages.lofire-demo;
          exePath = "/bin/lofire-demo";
        };
        default = lofire-node;
      };
    });
}
