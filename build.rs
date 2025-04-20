use std::{
    env::{self, current_dir},
    fs,
    path::Path,
    process::Command,
};

fn main() {
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "linux" => {
            let execve_host_path = "artifacts/linux_execve_host";
            println!("cargo:rerun-if-changed={}", execve_host_path);

            let cwd = current_dir().unwrap();
            let execve_host_path = cwd.join(execve_host_path);

            let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
            let execve_host_target = format!("{}-unknown-linux-musl", target_arch);
            let execve_host_build_args: &[&str] = match target_arch.as_str() {
                "aarch64" => &[
                    "-Zbuild-std=std,panic_abort",
                    "--target",
                    "aarch64-unknown-linux-musl.json",
                ],
                "x86_64" => &["--target", "x86_64-unknown-linux-musl"],
                _ => panic!("Unsuppported target arch: {}", target_arch),
            };

            let exit_status = Command::new("rustup")
                .current_dir(&execve_host_path)
                .args(["target", "add", &execve_host_target])
                .status()
                .unwrap();
            assert_eq!(exit_status.code(), Some(0));

            let out_dir = cwd.join(Path::new(&std::env::var_os("OUT_DIR").unwrap()));
            let execve_host_target_dir = out_dir.join("linux_execve_host_target");
            let mut execve_host_build_command = Command::new("cargo");
            execve_host_build_command
                .env_clear()
                .env("PATH", env::var_os("PATH").unwrap())
                .current_dir(&execve_host_path)
                .env("CARGO_TARGET_DIR", &execve_host_target_dir)
                .arg("build");
            if let Some(rustup_home_env) = env::var_os("RUSTUP_HOME") {
                execve_host_build_command.env("RUSTUP_HOME", rustup_home_env);
            }
            let is_release = env::var("PROFILE").unwrap() == "release";
            if is_release {
                execve_host_build_command.arg("--release");
            };
            let exit_status = dbg!(execve_host_build_command.args(execve_host_build_args))
                .status()
                .unwrap();
            assert_eq!(exit_status.code(), Some(0));

            fs::copy(
                execve_host_target_dir
                    .join(&execve_host_target)
                    .join(if is_release { "release" } else { "debug" })
                    .join("linux_execve_host"),
                out_dir.join("linux_execve_host"),
            )
            .unwrap();
        }
        other => panic!("Unsuppported target os: {}", other),
    }
}
