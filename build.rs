use std::env::current_dir;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-changed=preload_cdylib");

    let path_env = std::env::var_os("PATH").unwrap();
    let target = std::env::var("TARGET").unwrap();
    let is_debug = std::env::var_os("PROFILE").unwrap() == "debug";
    let out_dir = current_dir()
        .unwrap()
        .join(Path::new(&std::env::var_os("OUT_DIR").unwrap()));
    let cargo_target_dir = out_dir.join("preload_cdylib_target");

    let status = Command::new("cargo")
        .args([
            "build",
            "--profile",
            if is_debug { "dev" } else { "release" },
            "--target",
            target.as_str(),
        ])
        .current_dir("preload_cdylib")
        .env_clear()
        .env("PATH", path_env)
        .env("CARGO_TARGET_DIR", &cargo_target_dir)
        .status()
        .unwrap();
    assert!(status.success());
    fs::copy(
        cargo_target_dir
            .join(&target)
            .join(if is_debug { "debug" } else { "release" })
            .join(if cfg!(target_os = "macos") {
                "libpreload_cdylib.dylib"
            } else {
                "libpreload_cdylib.so"
            }),
        out_dir.join("preload_cdylib"),
    )
    .unwrap();
}
