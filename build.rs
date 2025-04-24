use std::{
    env::{self, current_dir},
    ffi::OsStr,
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
};

use const_fnv1a_hash::fnv1a_hash_str_128;

fn command_with_clean_env(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap());
    if let Some(rustup_home_env) = env::var_os("RUSTUP_HOME") {
        command.env("RUSTUP_HOME", rustup_home_env);
    }
    command
}

fn build_interpose() {
    let interpose_path = "interpose";
    println!("cargo:rerun-if-changed={}", interpose_path);

    let cwd = current_dir().unwrap();
    let interpose_path = cwd.join(interpose_path);

    let out_dir = cwd.join(Path::new(&std::env::var_os("OUT_DIR").unwrap()));
    let interpose_target_dir = out_dir.join("fspy_interpose_target");

    let mut build_cmd = command_with_clean_env("cargo");
    build_cmd
        .current_dir(&interpose_path)
        .env("CARGO_TARGET_DIR", &interpose_target_dir)
        .arg("build");

    // config target
    // build_cmd.args([
    //     "-Zbuild-std=std,panic_abort",
    //     "--target",
    //     "aarch64-unknown-linux-musl.json",
    // ])

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let (interpose_target, interpose_target_type, output_name) =
        match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
            "linux" => (
                match target_arch.as_str() {
                    "aarch64" | "x86_64" => format!("{}-unknown-linux-musl", &target_arch),
                    _ => panic!("Unsuppported linux target arch: {}", &target_arch),
                },
                "--bins",
                "libfsypy_interpose.so",
            ),
            "macos" => (
                env::var("TARGET").unwrap(),
                "--lib",
                "libfspy_interpose.dylib",
            ),
            "windows" => todo!(),
            other => panic!("Unsuppported target os: {}", other),
        };

    let rustup_exit_status = command_with_clean_env("rustup")
        .current_dir(&interpose_path)
        .args(["target", "add", &interpose_target])
        .status()
        .unwrap();
    assert_eq!(rustup_exit_status.code(), Some(0));

    if interpose_target == "aarch64-unknown-linux-musl" {
        build_cmd.args([
            "-Zbuild-std=std,panic_abort",
            "--target",
            "aarch64-unknown-linux-musl.json",
        ]);
    } else {
        build_cmd.args(["--target", &interpose_target]);
    }
    build_cmd.arg(interpose_target_type);

    let is_release = env::var("PROFILE").unwrap() == "release";
    if is_release {
        build_cmd.arg("--release");
    };
    let exit_status = dbg!(build_cmd).status().unwrap();
    assert_eq!(exit_status.code(), Some(0));

    fs::copy(
        interpose_target_dir
            .join(&interpose_target)
            .join(if is_release { "release" } else { "debug" })
            .join(output_name),
        out_dir.join("fspy_interpose"),
    )
    .unwrap();
}

fn ensure_downloaded(url: &str) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let filename = format!("{:x}", fnv1a_hash_str_128(url));
    let download_path = out_dir.join(&filename);
    if fs::exists(&download_path).unwrap() {
        return download_path;
    }
    let download_tmp_path = out_dir.join(format!("{}.tmp", filename));

    let resp = attohttpc::get(url).send().unwrap();
    assert_eq!(resp.status(), attohttpc::StatusCode::OK, "non-ok response from {}", url);
    resp.write_to(BufWriter::new(File::create(&download_tmp_path).unwrap()))
        .unwrap();
    fs::rename(&download_tmp_path, &download_path).unwrap();
    download_path
}

fn fetch_macos_binaries() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "macos" {
        return;
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let zsh_url = format!(
        "https://github.com/romkatv/zsh-bin/releases/download/v6.1.1/zsh-5.8-darwin-{}.tar.gz",
        if target_arch == "aarch64" {
            "arm64"
        } else {
            &target_arch
        }
    );
    let zsh_path = ensure_downloaded(&zsh_url);
}

fn main() {
    fetch_macos_binaries();
    build_interpose();
}
