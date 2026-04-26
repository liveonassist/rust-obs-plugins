{
  description = "Development environment for rust-obs-plugins";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };

        # Rust 2024 edition requires >= 1.85.
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust
            rustToolchain

            # bindgen / obs-sys build deps
            llvmPackages.libclang
            pkg-config

            # OBS itself — provides libobs + obs-frontend-api for linking
            # and the headers under its include/ tree.
            obs-studio

            # Submodule + general tooling
            git
            cmake

            # Python tooling for scripts/
            python312
            uv
          ];

          # bindgen needs to find libclang
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          shellHook = ''
            # Point obs-sys's bindgen at the OBS headers shipped by nixpkgs
            # so it doesn't need the obs-studio submodule checked out for
            # local development.
            export BINDGEN_EXTRA_CLANG_ARGS="-I${pkgs.obs-studio}/include/obs"

            echo "rust-obs-plugins dev shell"
            echo "  rustc:      $(rustc --version 2>/dev/null || echo 'not found')"
            echo "  obs-studio: ${pkgs.obs-studio.version}"
          '';
        };
      }
    );
}
