use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RIME_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=RIME_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG");

    if env::var_os("CARGO_FEATURE_NATIVE_RIME").is_none() {
        return;
    }

    match (env::var_os("RIME_INCLUDE_DIR"), env::var_os("RIME_LIB_DIR")) {
        (Some(include_dir), Some(lib_dir)) => {
            configure_from_env(PathBuf::from(include_dir), PathBuf::from(lib_dir));
        }
        (None, None) => configure_from_pkg_config(),
        _ => panic!(
            "native-rime requires both RIME_INCLUDE_DIR and RIME_LIB_DIR when explicit paths are used"
        ),
    }
}

fn configure_from_env(include_dir: PathBuf, lib_dir: PathBuf) {
    if !header_exists(&include_dir) {
        panic!(
            "RIME_INCLUDE_DIR must contain rime_api.h or rime/rime_api.h: {}",
            include_dir.display()
        );
    }
    if !lib_dir.is_dir() {
        panic!(
            "RIME_LIB_DIR must be an existing directory: {}",
            lib_dir.display()
        );
    }

    println!("cargo:metadata=rime_include={}", include_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=rime");
}

fn header_exists(include_dir: &Path) -> bool {
    include_dir.join("rime_api.h").is_file()
        || include_dir.join("rime").join("rime_api.h").is_file()
}

fn configure_from_pkg_config() {
    let pkg_config = env::var_os("PKG_CONFIG").unwrap_or_else(|| "pkg-config".into());
    let output = Command::new(pkg_config)
        .args(["--libs", "--cflags", "rime"])
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run pkg-config for native-rime; set RIME_INCLUDE_DIR and RIME_LIB_DIR instead: {error}"
            )
        });

    if !output.status.success() {
        panic!(
            "pkg-config could not find librime; set RIME_INCLUDE_DIR and RIME_LIB_DIR instead:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let flags = String::from_utf8_lossy(&output.stdout);
    let mut linked = false;
    for flag in flags.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-I") {
            println!("cargo:metadata=rime_include={path}");
        } else if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(name) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={name}");
            linked = true;
        }
    }

    if !linked {
        println!("cargo:rustc-link-lib=rime");
    }
}
