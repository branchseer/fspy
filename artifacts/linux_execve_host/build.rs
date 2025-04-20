use std::env;

fn main() {
    println!("cargo:rerun-if-changed=libreflect");

    cc::Build::new()
        .emit_rerun_if_env_changed(false)
        .pic(true)
        .include("libreflect/include")
        .include(format!("libreflect/arch/linux/{}", env::var("CARGO_CFG_TARGET_ARCH").unwrap()))
        .file("libreflect/src/exec.c")
        .file("libreflect/src/map_elf.c")
        .file("libreflect/src/stack_setup.c")
        .file("libreflect/src/jump.c")
        .compile("reflect");
}
