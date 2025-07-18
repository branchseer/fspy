use std::env;

fn main() {
    println!("cargo::rerun-if-changed=libreflect");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    cc::Build::new()
        .include("libreflect/include")
        .include(format!("libreflect/arch/linux/{}", arch))
        .file("libreflect/src/exec.c")
        .file("libreflect/src/jump.c")
        .file("libreflect/src/map_elf.c")
        .file("libreflect/src/stack_setup.c")
        .pic(true)
        .compile("reflect");
}
