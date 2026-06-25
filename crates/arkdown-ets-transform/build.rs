use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ARKDOWN_YUKU_ROOT");
    println!("cargo:rerun-if-changed=native/build.zig");
    println!("cargo:rerun-if-changed=native/yuku_module_analyzer.zig");
    println!("cargo:rustc-check-cfg=cfg(arkdown_yuku_native)");

    let Some(yuku_root) = env::var_os("ARKDOWN_YUKU_ROOT").map(PathBuf::from) else {
        return;
    };
    if !yuku_root.join("src/parser/root.zig").is_file() {
        println!(
            "cargo:warning=ARKDOWN_YUKU_ROOT does not point to a yuku checkout with src/parser/root.zig"
        );
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let native_dir = manifest_dir.join("native");
    let prefix = out_dir.join("yuku-native");
    let cache_dir = out_dir.join("zig-cache");
    let global_cache_dir = out_dir.join("zig-global-cache");

    std::fs::create_dir_all(&cache_dir).expect("create zig cache dir");
    std::fs::create_dir_all(&global_cache_dir).expect("create zig global cache dir");

    let status = Command::new("zig")
        .current_dir(&native_dir)
        .arg("build")
        .arg("-Doptimize=ReleaseFast")
        .arg(format!("-Dyuku-root={}", yuku_root.display()))
        .arg("--cache-dir")
        .arg(&cache_dir)
        .arg("--global-cache-dir")
        .arg(&global_cache_dir)
        .arg("--prefix")
        .arg(&prefix)
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-cfg=arkdown_yuku_native");
            println!(
                "cargo:rustc-link-search=native={}",
                prefix.join("lib").display()
            );
            println!("cargo:rustc-link-lib=static=arkdown_yuku_module_analyzer");
            println!(
                "cargo:rerun-if-changed={}",
                yuku_root.join("src/parser").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                yuku_root.join("src/util").display()
            );
        }
        Ok(status) => {
            panic!("failed to build yuku native module analyzer: zig exited with {status}");
        }
        Err(error) if is_not_found(&error) => {
            println!("cargo:warning=zig not found; yuku native module analyzer disabled");
        }
        Err(error) => {
            panic!("failed to run zig for yuku native module analyzer: {error}");
        }
    }
}

fn is_not_found(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}
