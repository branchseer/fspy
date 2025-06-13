use std::{
    env::{self, current_dir},
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::Path,
    process::Command,
};

use anyhow::{bail, Context};
use xxhash_rust::xxh3::xxh3_128;

fn command_with_clean_env(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    let mut envs = env::vars_os().collect::<Vec<(OsString, OsString)>>();
    envs.retain(|(name, _)| if let Some(name_str) = name.to_str() {
        !(name_str.starts_with("CARGO_") || name_str.starts_with("cargo_"))
    } else {
        true
    });
    command
        .env_clear().envs(envs);
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
                "fspy_interpose",
            ),
            "macos" => (
                env::var("TARGET").unwrap(),
                "--lib",
                "libfspy_interpose.dylib",
            ),
            "windows" => (
                // TODO: try gnullvm for cross-compiling
                format!("{}-pc-windows-msvc", &target_arch),
                "--lib",
                "fspy_interpose.dll"
            ),
            other => panic!("Unsuppported target os: {}", other),
        };

    // let rustup_exit_status = command_with_clean_env("rustup")
    //     .current_dir(&interpose_path)
    //     .args(["target", "add", &interpose_target])
    //     .status()
    //     .unwrap();
    // assert_eq!(rustup_exit_status.code(), Some(0));

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

    let interpose_path = out_dir.join("fspy_interpose");
    let interpose_hash_path = out_dir.join("fspy_interpose.hash");

    let interpose_data = fs::read(
        dbg!(interpose_target_dir
            .join(&interpose_target)
            .join(if is_release { "release" } else { "debug" })
            .join(output_name)),
    )
    .unwrap();
    let interpose_hash = xxh3_128(&interpose_data);

    fs::write(&interpose_path, interpose_data).unwrap();

    fs::write(&interpose_hash_path, format!("{:x}", interpose_hash)).unwrap();

    // fs::copy(
    //     interpose_target_dir
    //         .join(&interpose_target)
    //         .join(if is_release { "release" } else { "debug" })
    //         .join(output_name),
    //     interpose_path,
    // )
    // .unwrap();
}

fn download(url: &str) -> anyhow::Result<impl Read + use<>> {
    let resp = attohttpc::get(url).send().unwrap();
    if resp.status() != attohttpc::StatusCode::OK {
        bail!("non-ok response: {:?}", resp.status())
    }
    Ok(resp)
}

fn unpack_tar_gz(content: impl Read, path: &str) -> anyhow::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    // let path = path.as_ref();
    let tar = GzDecoder::new(content);
    let mut archive = Archive::new(tar);
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path_bytes().as_ref() == path.as_bytes() {
            let mut data = Vec::<u8>::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut data)?;
            return Ok(data);
        }
    }
    bail!("Path {} not found in tar gz", path)
}

fn download_and_unpack_tar_gz(url: &str, path: &str) -> anyhow::Result<Vec<u8>> {
    let resp = download(url).context(format!("Failed to get ok response from {}", url))?;
    let data = unpack_tar_gz(resp, path).context(format!(
        "Failed to download or unpack {} out of {}",
        path, url
    ))?;
    Ok(data)
}

const MACOS_BINARY_DOWNLOADS: &[(&str, &[(&str, &str, u128)])] = &[
    (
        "aarch64",
        &[(
            "https://github.com/branchseer/oils-for-unix-binaries/releases/download/0.29.0-manual/oils-for-unix-0.29.0-aarch64-apple-darwin.tar.gz",
            "oils-for-unix",
            149945237112824769531360595981178091193,
        ),
        (
            "https://github.com/uutils/coreutils/releases/download/0.1.0/coreutils-0.1.0-aarch64-apple-darwin.tar.gz",
            "coreutils-0.1.0-aarch64-apple-darwin/coreutils",
            255656813290649147736009964224176006890,
        )],
    ),
    (
        "x86_64",
        &[(
            "https://github.com/branchseer/oils-for-unix-binaries/releases/download/0.29.0-manual/oils-for-unix-0.29.0-x86_64-apple-darwin.tar.gz",
            "oils-for-unix",
            286203014616009968685843701528129413859,
        ),
        (
            "https://github.com/uutils/coreutils/releases/download/0.1.0/coreutils-0.1.0-x86_64-apple-darwin.tar.gz",
            "coreutils-0.1.0-x86_64-apple-darwin/coreutils",
            75344743234387926348628744659874018387,
        )],
    )
];

fn fetch_macos_binaries() -> anyhow::Result<()> {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "macos" {
        return Ok(());
    };
    let out_dir = current_dir()
        .unwrap()
        .join(Path::new(&std::env::var_os("OUT_DIR").unwrap()));

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let downloads = MACOS_BINARY_DOWNLOADS
        .iter()
        .find(|(arch, _)| *arch == target_arch)
        .context(format!("Unsupported macOS arch: {}", target_arch))?
        .1;
    // let downloads = [(zsh_url.as_str(), "bin/zsh", zsh_hash)];
    for (url, path_in_targz, expected_hash) in downloads.iter().copied() {
        let filename = path_in_targz.split('/').rev().next().unwrap();
        let download_path = out_dir.join(filename);
        let hash_path = out_dir.join(format!("{}.hash", filename));

        let file_exists = matches!(fs::read(&download_path), Ok(existing_file_data) if xxh3_128(&existing_file_data) == expected_hash);
        if !file_exists {
            let data = download_and_unpack_tar_gz(url, path_in_targz)?;
            fs::write(&download_path, &data).context(format!(
                "Saving {path_in_targz} in {url} to {}",
                download_path.display()
            ))?;
            let actual_hash = xxh3_128(&data);
            assert_eq!(
                actual_hash, expected_hash,
                "expected_hash of {} in {} needs to be updated",
                path_in_targz, url
            );
        }
        fs::write(&hash_path, format!("{:x}", expected_hash))?;
    }
    Ok(())
    // let zsh_path = ensure_downloaded(&zsh_url);
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    fetch_macos_binaries().context("Failed to fetch macOS binaries")?;
    build_interpose();
    Ok(())
}
