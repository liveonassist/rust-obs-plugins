{
  description = "Development environment for rust-obs-plugins";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Track obs-studio from nixpkgs master so we can pick up new OBS
    # releases ahead of the nixos-unstable channel cadence.
    nixpkgs-master.url = "github:NixOS/nixpkgs/master";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    nixpkgs-master,
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
        pkgsMaster = import nixpkgs-master {
          inherit system;
          config.allowUnfree = true;
        };

        # obs-studio pulled from nixpkgs master to stay close to upstream.
        obs-studio = pkgsMaster.obs-studio;

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

            # OBS — provides libobs + obs-frontend-api .so files for linking.
            # Headers come from the obs-sys/obs submodule (pinned in this
            # repo) so the OBS version we build against is tracked in git,
            # not coupled to whatever nixpkgs ships.
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
            echo "🎬 rust-obs-plugins development environment loaded!"
            echo ""
            echo "Language runtimes:"
            echo "  - 🦀 Rust:   $(rustc --version 2>/dev/null || echo 'not found')"
            echo "  - 🐍 Python: $(python --version 2>/dev/null || echo 'not found')"
            echo ""
            echo "OBS:"
            echo "  - 🔗 Linking against obs-studio ${obs-studio.version} (from nixpkgs-master)"
            if [ -f obs-sys/obs/libobs/obs.h ]; then
              submodule_rev="$(git -C obs-sys/obs rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
              echo "  - 📌 Headers from obs-sys/obs submodule @ $submodule_rev"
            else
              echo "  - ⚠️  obs-sys/obs submodule not initialized — bindgen will use pre-generated bindings"
              echo "       run: git submodule update --init --recursive"
            fi
          '';
        };
      }
    );
}
