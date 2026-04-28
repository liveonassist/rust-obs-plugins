use std::env;
use std::fs;
use std::path::PathBuf;

fn install_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from(&*shellexpand::tilde("~/Applications/OBS.app")),
        PathBuf::from("/Applications/OBS.app"),
    ]
}

pub fn find_mac_obs_lib() {
    if let Some(path) = env::var("LIBOBS_PATH").ok() {
        println!("cargo:rustc-link-search=native={}", path);
        return;
    }

    let candidates: Vec<PathBuf> = install_roots()
        .into_iter()
        .flat_map(|root| {
            [
                root.join("Contents/MacOS"),
                root.join("Contents/Frameworks"),
            ]
        })
        .collect();

    let mut found_obs = false;
    let mut found_obs_frontend = false;

    for c in candidates.iter() {
        if !found_obs {
            if let Ok(meta) = fs::metadata(c.join("libobs.0.dylib")) {
                if meta.is_file() {
                    println!("cargo:rustc-link-search={}", c.display());
                    println!("cargo:rustc-link-lib=dylib=obs.0");
                    found_obs = true;
                }
            }

            if let Ok(meta) = fs::metadata(c.join("libobs.framework")) {
                if meta.is_dir() {
                    println!("cargo:rustc-link-search=framework={}", c.display());
                    println!("cargo:rustc-link-lib=framework=libobs");
                    found_obs = true;
                }
            }
        }

        if !found_obs_frontend {
            if let Ok(meta) = fs::metadata(c.join("libobs-frontend-api.1.dylib")) {
                if meta.is_file() {
                    println!("cargo:rustc-link-search={}", c.display());
                    println!("cargo:rustc-link-lib=dylib=obs-frontend-api.1");
                    found_obs_frontend = true;
                }
            }
        }
    }

    if !found_obs {
        panic!("could not find libobs - install OBS or set LIBOBS_PATH");
    }

    if !found_obs_frontend {
        panic!("could not find libobs-frontend-api - install OBS or set LIBOBS_PATH");
    }
}

pub fn detect_obs_major() -> Option<u32> {
    for root in install_roots() {
        let plist = root.join("Contents/Info.plist");
        let Ok(contents) = fs::read_to_string(&plist) else {
            continue;
        };
        // Info.plist is XML; find the value following the CFBundleShortVersionString key.
        if let Some(major) = parse_short_version_major(&contents) {
            return Some(major);
        }
    }
    None
}

fn parse_short_version_major(plist: &str) -> Option<u32> {
    let key = "<key>CFBundleShortVersionString</key>";
    let after_key = plist.split_once(key)?.1;
    let open = after_key.find("<string>")?;
    let rest = &after_key[open + "<string>".len()..];
    let close = rest.find("</string>")?;
    rest[..close].trim().split('.').next()?.parse().ok()
}
