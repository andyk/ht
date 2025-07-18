{
  description = "ht";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        ht = pkgs.rustPlatform.buildRustPackage {
          pname = "ht";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = self;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.darwin.apple_sdk.frameworks.Foundation
          ];
        };
      in
      {
        packages = {
          ht = ht;
          default = ht;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs =
            with pkgs;
            [
              (rust-bin.stable."1.74.0".default.override { extensions = [ "rust-src" ]; })
              bashInteractive
            ]
            ++ (lib.optionals stdenv.isDarwin [
              libiconv
              darwin.apple_sdk.frameworks.Foundation
            ]);
        };
      }
    );
}
