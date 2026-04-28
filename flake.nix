{
  description = "Development environment for rust-obs-plugins";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Track obs-studio from nixpkgs master so we can pick up new OBS
    # releases ahead of the nixos-unstable channel cadence. The default
    # dev shell links against this (currently the v32.x line).
    nixpkgs-master.url = "github:NixOS/nixpkgs/master";

    # Pinned nixpkgs revisions for older OBS major lines. Commits sourced
    # from https://lazamar.co.uk/nix-versions/?package=obs-studio — picking
    # the latest release on each line that lazamar still has indexed.
    nixpkgs-obs-v30.url = "github:NixOS/nixpkgs/21808d22b1cda1898b71cf1a1beb524a97add2c4"; # obs-studio 30.2.3
    nixpkgs-obs-v31.url = "github:NixOS/nixpkgs/e6f23dc08d3624daab7094b701aa3954923c6bbb"; # obs-studio 31.0.3

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
    nixpkgs-obs-v30,
    nixpkgs-obs-v31,
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
        importPinned = src:
          import src {
            inherit system;
            config.allowUnfree = true;
          };
        pkgsObsV30 = importPinned nixpkgs-obs-v30;
        pkgsObsV31 = importPinned nixpkgs-obs-v31;
        pkgsObsV32 = importPinned nixpkgs-master;

        # nixpkgs's obs-studio is linux-only. On darwin we expect the user (or
        # CI) to install OBS.app into /Applications; obs-sys-rs/build_mac.rs
        # already finds libobs / obs-frontend-api there. Gate the attribute
        # access behind the platform check so we don't trip the platform
        # assertion when evaluating the flake on darwin.
        obsPkgIfLinux = pkgsPinned:
          if pkgs.stdenv.isLinux
          then pkgsPinned.obs-studio
          else null;

        # Pinned so the dev shell and CI agree exactly. Bump in lockstep with
        # the dtolnay/rust-toolchain pin in .github/workflows/build.yml.
        rustToolchain = pkgs.rust-bin.stable."1.95.0".default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        };

        mkObsShell = {
          major,
          obsPkg,
        }:
          pkgs.mkShell {
            buildInputs = with pkgs;
              [
                # Rust
                rustToolchain

                # bindgen / obs-sys-rs build deps
                llvmPackages.libclang
                pkg-config

                # simde — vendored by OBS in v30/v31 but unbundled in v32, so
                # we always carry the standalone package. Header-only, so it
                # builds on darwin too.
                simde

                # Submodule + general tooling
                git
                cmake

                # Python tooling for scripts/
                python312
                uv
              ]
              # OBS — provides libobs + obs-frontend-api .so files for linking
              # on Linux. Headers come from obs-sys-rs/obs-v${toString major} so
              # the version we build against is tracked in git, not coupled to
              # whatever nixpkgs ships. On darwin the package isn't available
              # in nixpkgs; obs-sys-rs/build_mac.rs locates libobs in
              # /Applications/OBS.app instead.
              ++ pkgs.lib.optional (obsPkg != null) obsPkg;

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            # Consumed by obs-sys-rs/build.rs to assert the linked libobs major
            # matches the selected `obs-${major}` Cargo feature.
            OBS_LIBRARY_MAJOR_VER = toString major;

            # Bindgen needs to find <simde/x86/sse2.h> (vendored by OBS but
            # not exposed by the obs-studio derivation).
            SIMDE_INCLUDE_DIR = "${pkgs.simde}/include";

            shellHook = ''
              # libclang doesn't pick up the system header paths by default —
              # bindgen needs them to resolve <sys/types.h> et al. Reuse the
              # cflags the wrapped cc derivation already exposes. glibc.dev is
              # linux-only; on darwin the SDK include path is already in
              # cc-cflags via the apple-sdk wrapping.
              export BINDGEN_EXTRA_CLANG_ARGS="$(< ${pkgs.stdenv.cc}/nix-support/libc-crt1-cflags) \
                $(< ${pkgs.stdenv.cc}/nix-support/libc-cflags) \
                $(< ${pkgs.stdenv.cc}/nix-support/cc-cflags) \
                $(< ${pkgs.stdenv.cc}/nix-support/libcxx-cxxflags)${pkgs.lib.optionalString pkgs.stdenv.isLinux " -idirafter ${pkgs.glibc.dev}/include"}"

              echo "🎬 rust-obs-plugins development environment loaded! (OBS v${toString major} target)"
              echo ""
              echo "Language runtimes:"
              echo "  - 🦀 Rust:   $(rustc --version 2>/dev/null || echo 'not found')"
              echo "  - 🐍 Python: $(python --version 2>/dev/null || echo 'not found')"
              echo ""
              echo "OBS:"
              ${
                if obsPkg != null
                then ''echo "  - 🔗 Linking against obs-studio ${obsPkg.version}"''
                else ''echo "  - 🔗 Linking against /Applications/OBS.app (install OBS v${toString major}.x manually — see build_mac.rs)"''
              }
              if [ -f obs-sys-rs/obs-v${toString major}/libobs/obs-config.h ]; then
                submodule_rev="$(git -C obs-sys-rs/obs-v${toString major} rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
                echo "  - 📌 Headers from obs-sys-rs/obs-v${toString major} submodule @ $submodule_rev"
              else
                echo "  - ⚠️  obs-sys-rs/obs-v${toString major} submodule not initialized"
                echo "       run: git submodule update --init obs-sys-rs/obs-v${toString major}"
              fi
              echo ""
              echo "Build with:"
              echo "  cargo build --no-default-features --features obs-${toString major} --workspace"
            '';
          };
      in {
        devShells = {
          obs-v30 = mkObsShell {
            major = 30;
            obsPkg = obsPkgIfLinux pkgsObsV30;
          };
          obs-v31 = mkObsShell {
            major = 31;
            obsPkg = obsPkgIfLinux pkgsObsV31;
          };
          obs-v32 = mkObsShell {
            major = 32;
            obsPkg = obsPkgIfLinux pkgsObsV32;
          };
          default = mkObsShell {
            major = 32;
            obsPkg = obsPkgIfLinux pkgsObsV32;
          };
        };
      }
    );
}
