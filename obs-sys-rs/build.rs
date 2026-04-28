use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod build_win;

#[cfg(target_os = "macos")]
mod build_mac;

const SUPPORTED_MAJORS: &[u32] = &[30, 31, 32];

fn selected_obs_major() -> u32 {
    let enabled: Vec<u32> = SUPPORTED_MAJORS
        .iter()
        .copied()
        .filter(|m| env::var(format!("CARGO_FEATURE_OBS_{m}")).is_ok())
        .collect();

    match enabled.as_slice() {
        [m] => *m,
        [] => panic!(
            "obs-sys-rs: no OBS version feature enabled. Enable exactly one of: {}.",
            SUPPORTED_MAJORS
                .iter()
                .map(|m| format!("obs-{m}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        many => panic!(
            "obs-sys-rs: multiple OBS version features enabled ({}). Enable exactly one. \
             If you depend on obs-rs from a workspace, set `default-features = false` \
             on every member that pulls it in and pick the same `obs-XX` feature everywhere.",
            many.iter()
                .map(|m| format!("obs-{m}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn submodule_dir(major: u32) -> PathBuf {
    PathBuf::from(format!("./obs-v{major}"))
}

struct ObsVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

fn read_header_version(submodule: &Path) -> ObsVersion {
    let config_path = submodule.join("libobs/obs-config.h");
    let contents = fs::read_to_string(&config_path).unwrap_or_else(|_| {
        panic!(
            "obs-sys-rs: cannot read {} — submodule not initialized? \
             Run: git submodule update --init {}",
            config_path.display(),
            submodule.display()
        )
    });

    fn grab(contents: &str, key: &str) -> u32 {
        for line in contents.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(&format!("#define {key} ")) {
                return rest.trim().parse().unwrap_or_else(|_| {
                    panic!("obs-sys-rs: failed to parse {key} from obs-config.h")
                });
            }
        }
        panic!("obs-sys-rs: {key} not found in obs-config.h");
    }

    ObsVersion {
        major: grab(&contents, "LIBOBS_API_MAJOR_VER"),
        minor: grab(&contents, "LIBOBS_API_MINOR_VER"),
        patch: grab(&contents, "LIBOBS_API_PATCH_VER"),
    }
}

fn detect_linked_obs_major() -> Option<u32> {
    if let Ok(v) = env::var("OBS_LIBRARY_MAJOR_VER") {
        return v.trim().parse().ok();
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(m) = linux_detect_libobs_major() {
            return Some(m);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(m) = build_mac::detect_obs_major() {
            return Some(m);
        }
    }

    #[cfg(windows)]
    {
        if let Some(m) = build_win::detect_obs_major() {
            return Some(m);
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn linux_detect_libobs_major() -> Option<u32> {
    use std::process::Command;

    if let Ok(out) = Command::new("pkg-config")
        .args(["--modversion", "libobs"])
        .output()
        && out.status.success()
    {
        let v = String::from_utf8_lossy(&out.stdout);
        if let Some(major) = v.split('.').next().and_then(|s| s.trim().parse().ok()) {
            return Some(major);
        }
    }

    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(extra) = env::var("LD_LIBRARY_PATH") {
        search_dirs.extend(
            extra
                .split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        );
    }
    search_dirs.extend(
        ["/usr/lib", "/usr/local/lib", "/usr/lib/x86_64-linux-gnu"]
            .iter()
            .map(PathBuf::from),
    );

    for dir in search_dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(rest) = name.strip_prefix("libobs.so.")
                && let Some(major) = rest.split('.').next().and_then(|s| s.parse().ok())
            {
                return Some(major);
            }
        }
    }

    None
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DONT_USE_GENERATED_BINDINGS");
    println!("cargo:rerun-if-env-changed=OBS_LIBRARY_MAJOR_VER");
    println!("cargo:rerun-if-env-changed=SIMDE_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    for m in SUPPORTED_MAJORS {
        println!("cargo:rerun-if-env-changed=CARGO_FEATURE_OBS_{m}");
    }

    let major = selected_obs_major();
    let submodule = submodule_dir(major);

    let header_version = read_header_version(&submodule);
    if header_version.major != major {
        panic!(
            "obs-sys-rs: feature obs-{major} is enabled but {}/libobs/obs-config.h reports \
             OBS {}.{}.{}. The submodule is checked out at the wrong tag — fix with: \
             git -C {} checkout <tag-in-the-{major}.x.y-line>",
            submodule.display(),
            header_version.major,
            header_version.minor,
            header_version.patch,
            submodule.display(),
        );
    }

    if let Some(linked_major) = detect_linked_obs_major() {
        if linked_major != major {
            panic!(
                "obs-sys-rs: feature obs-{major} is enabled but the linked libobs reports major \
                 version {linked_major}. Either switch features (--no-default-features --features \
                 obs-{linked_major}) or enter the matching dev shell (nix develop \
                 .#obs-v{linked_major}). To override the detected value set \
                 OBS_LIBRARY_MAJOR_VER=<major>."
            );
        }
    } else {
        println!(
            "cargo:warning=obs-sys-rs: could not determine linked libobs major version; \
             skipping link-time version check (set OBS_LIBRARY_MAJOR_VER to silence this)."
        );
    }

    println!("cargo:rustc-cfg=obs_major=\"{major}\"");
    println!(
        "cargo:rustc-cfg=obs_version=\"{}.{}.{}\"",
        header_version.major, header_version.minor, header_version.patch
    );
    println!("cargo:rustc-check-cfg=cfg(obs_major, values(\"30\", \"31\", \"32\"))");
    println!("cargo:rustc-check-cfg=cfg(obs_version, values(any()))");
    println!("cargo:rustc-env=OBS_TARGET_MAJOR={major}");
    println!(
        "cargo:rustc-env=OBS_TARGET_VERSION={}.{}.{}",
        header_version.major, header_version.minor, header_version.patch
    );

    #[cfg(not(target_os = "macos"))]
    {
        println!("cargo:rustc-link-lib=dylib=obs");
        println!("cargo:rustc-link-lib=dylib=obs-frontend-api");
    }

    #[cfg(target_os = "macos")]
    build_mac::find_mac_obs_lib();

    #[cfg(windows)]
    build_win::find_windows_obs_lib();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");

    let mut clang_args: Vec<String> = Vec::new();
    // Windows has an issue with the _udiv128 function not being declared
    // So just ignore for now!
    #[cfg(windows)]
    clang_args.push("-Wno-error=implicit-function-declaration".to_string());

    // Submodule headers come first so the version-pinned source wins for any
    // file that exists in both places.
    clang_args.push(format!("-I{}/libobs/", submodule.display()));
    // The frontend-api header moved between OBS releases: v30 keeps it under
    // UI/obs-frontend-api/, v31+ relocated it to frontend/api/. Add whichever
    // directory actually exists in the pinned submodule.
    for rel in ["UI/obs-frontend-api", "frontend/api"] {
        let dir = submodule.join(rel);
        if dir.is_dir() {
            clang_args.push(format!("-I{}", dir.display()));
        }
    }

    // Fold in include paths from the linked libobs (via pkg-config). This is
    // how we pick up obsconfig.h, which is generated by OBS's own cmake build
    // and isn't present in the source submodule.
    if let Ok(out) = std::process::Command::new("pkg-config")
        .args(["--cflags-only-I", "libobs"])
        .output()
        && out.status.success()
    {
        for token in String::from_utf8_lossy(&out.stdout).split_whitespace() {
            if let Some(path) = token.strip_prefix("-I") {
                clang_args.push(format!("-I{path}"));
                // Many OBS Linux packages drop the libobs headers under
                // <inc>/obs/, where obsconfig.h sits next to obs.h. Add
                // that subdir too so `#include "obsconfig.h"` resolves.
                let obs_subdir = Path::new(path).join("obs");
                if obs_subdir.is_dir() {
                    clang_args.push(format!("-I{}", obs_subdir.display()));
                }
            }
        }
    }

    // simde is a vendored build-time dep of OBS; the source submodule doesn't
    // ship its headers. Nix exposes it as a separate package — the obs-v* dev
    // shells set SIMDE_INCLUDE_DIR for us. Ignored on platforms where simde
    // headers come bundled with the OBS install.
    if let Ok(simde) = env::var("SIMDE_INCLUDE_DIR") {
        clang_args.push(format!("-I{simde}"));
    }

    // obs-config.h has `#include "obsconfig.h"` (unconditional from v31 on).
    // obsconfig.h is generated by OBS's own cmake from obsconfig.h.in, so it's
    // missing from both the source submodule and the user-facing OBS installs
    // on macOS/Windows. On Linux the pkg-config -I above already points at the
    // real one. Drop a stub in OUT_DIR as a last-resort include path so bindgen
    // can preprocess the OBS public headers — the macros it defines aren't
    // referenced by anything bindgen emits.
    let stub_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("obs-stub");
    fs::create_dir_all(&stub_dir).expect("create obs-stub dir");
    fs::write(
        stub_dir.join("obsconfig.h"),
        "#pragma once\n\
         #define OBS_DATA_PATH \"../../data\"\n\
         #define OBS_PLUGIN_PATH \"../../obs-plugins/%module%\"\n\
         #define OBS_PLUGIN_DESTINATION \"obs-plugins\"\n\
         #define OBS_INSTALL_PREFIX \"\"\n\
         #define OBS_RELATIVE_PREFIX \"../../\"\n\
         #define OBS_VERSION \"unknown\"\n\
         #define OBS_RELEASE_CANDIDATE 0\n\
         #define OBS_BETA 0\n",
    )
    .expect("write obsconfig.h stub");
    clang_args.push(format!("-I{}", stub_dir.display()));

    let builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_args(&clang_args)
        .blocklist_type("_bindgen_ty_2")
        .blocklist_type("_bindgen_ty_3")
        .blocklist_type("_bindgen_ty_4")
        // ARM NEON multi-vector aggregate types (e.g. int8x8x2_t) leak in from
        // <arm_neon.h> on aarch64 macOS. bindgen emits a layout assertion of
        // the form `align_of::<T>() - 8usize` that underflows in const eval
        // for these types. OBS doesn't expose them in its public API, so just
        // drop them from the generated bindings. Includes ARMv9 FP8 ("modal
        // float", mfloat8x…) variants pulled in by newer Xcode toolchains.
        .blocklist_type("(u?int|m?float|bfloat|poly)[0-9]+x[0-9]+x[0-9]+_t")
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    match builder.generate() {
        Ok(bindings) => {
            bindings
                .write_to_file(&out_path)
                .expect("Couldn't write bindings!");
            // Only refresh the in-tree fallback when building the default major
            // (latest stable). Older majors don't have committed fallbacks.
            if major == *SUPPORTED_MAJORS.last().unwrap() {
                fs::copy(&out_path, "generated/bindings.rs").expect("Could not copy bindings!");
            }
        }

        Err(e) => {
            if env::var("DONT_USE_GENERATED_BINDINGS").is_ok() {
                panic!("Failed to generate headers with bindgen: {}", e);
            }

            if major != *SUPPORTED_MAJORS.last().unwrap() {
                panic!(
                    "obs-sys-rs: bindgen failed for obs-{major} and the pre-generated fallback only \
                     covers obs-{}. Initialize the submodule (git submodule update --init {}) \
                     and ensure libclang is available. Original error: {e}",
                    SUPPORTED_MAJORS.last().unwrap(),
                    submodule.display(),
                );
            }

            println!("cargo:warning=Could not find obs headers - using pre-compiled.");
            println!("cargo:warning=This could result in a library that doesn't work.");
            fs::copy("generated/bindings.rs", out_path).expect("Could not copy bindings!");
        }
    }
}
