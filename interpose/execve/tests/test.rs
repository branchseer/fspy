use std::{
    ffi::{CStr, CString},
    io::{self, Read as _, Write as _},
    os::{fd::AsRawFd as _, raw::c_int, unix::ffi::OsStrExt as _},
    path::Path,
    process::{Command, Stdio},
    ptr::null,
};

use nix::libc::{STDERR_FILENO, STDOUT_FILENO, dup2, fork, waitpid};
use tempfile::tempdir;

fn get_output(action: impl FnOnce()) -> io::Result<String> {
    let (mut reader, writer) = std::io::pipe()?;
    let child_pid = unsafe { fork() };
    if child_pid == -1 {
        return Err(io::Error::last_os_error());
    }
    if child_pid == 0 {
        // Safety: this is in another process
        unsafe {
            dup2(writer.as_raw_fd(), STDOUT_FILENO);
            dup2(writer.as_raw_fd(), STDERR_FILENO);
            action();
        }
    } else {
        let mut status: c_int = 0;
        let ret = unsafe { waitpid(child_pid, &mut status, 0) };
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        assert_eq!(status, 0);
    }
    drop(writer);
    let mut stdout = String::new();
    reader.read_to_string(&mut stdout)?;
    Ok(stdout)
}

fn with_cc(
    musl: bool,
    source_in_main: &str,
    cc_args: impl IntoIterator<Item = &'static str>,
    f: impl FnOnce(&Path),
) -> io::Result<()> {
    let source = format!(
        "#include <stdio.h>\nextern char **environ;\nint main(int argc, char *argv[]) {{ {} }}",
        source_in_main
    );
    let tmpdir = tempdir()?;
    let mut cmd = Command::new(if musl { "musl-gcc" } else { "cc" })
        .args(cc_args.into_iter().chain(["-x", "c", "-"]))
        .current_dir(&tmpdir)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = cmd.stdin.take().unwrap();
    stdin.write_all(source.as_bytes())?;
    drop(stdin);
    let exit_status = cmd.wait()?;
    assert_eq!(exit_status.code(), Some(0));

    let bin_path = tmpdir.path().join("a.out");
    f(&bin_path);
    Ok(())
}

const PRINT_ARGV_ENVS: &str = r#"for (int i = 0; i < argc; ++i) printf(" %s", argv[i]); for (int i=0; environ[i]; ++i) printf(" %s", environ[i]);"#;

#[test]
fn pie() {
    with_cc(
        false,
        &format!("printf(\"pie\"); {}", PRINT_ARGV_ENVS),
        ["-fPIE"],
        |bin_path| {
            let env = &[
                c"env1=1".as_ptr(),
                c"env2=2".as_ptr(),
                c"env3=3".as_ptr(),
                null(),
            ];
            let stdout = get_output(|| unsafe {
                let result = execve::execve(
                    bin_path,
                    [c"arg0", c"arg1"].into_iter(),
                    [c"env1=1", c"env2=2"].into_iter(),
                );
                match result.unwrap() {}
            })
            .unwrap();
            assert_eq!(stdout, "pie arg0 arg1 env1=1 env2=2");
        },
    )
    .unwrap();
}

#[test]
fn non_pie() {
    with_cc(
        false,
        &format!("printf(\"non_pie\"); {}", PRINT_ARGV_ENVS),
        ["-no-pie"],
        |bin_path| {
            let stdout = get_output(|| unsafe {
                let result = execve::execve(
                    bin_path,
                    [c"arg0", c"arg1"].into_iter(),
                    [c"env1=1", c"env2=2"].into_iter(),
                );
                match result.unwrap() {}
            })
            .unwrap();
            assert_eq!(stdout, "non_pie arg0 arg1 env1=1 env2=2");
        },
    )
    .unwrap();
}


#[test]
fn musl_static() {
    with_cc(
        true,
        &format!("printf(\"musl_static\"); {}", PRINT_ARGV_ENVS),
        ["-static"],
        |bin_path| {
            let stdout = get_output(|| unsafe {
                let result = execve::execve(
                    bin_path,
                    [c"arg0", c"arg1"].into_iter(),
                    [c"env1=1", c"env2=2"].into_iter(),
                );
                match result.unwrap() {}
            })
            .unwrap();
            assert_eq!(stdout, "musl_static arg0 arg1 env1=1 env2=2");
        },
    )
    .unwrap();
}
