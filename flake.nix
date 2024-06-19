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
      in
      {
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
