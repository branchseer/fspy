use std::ptr::null;

use arrayvec::ArrayVec;
use libc::c_char;

use crate::{client::Client, consts::ENVNAME_PROGRAM, nul::{NulTerminated, ThinCStr}, PATH_MAX};

pub struct ExecParams<'a> {
    argv: Option<NulTerminated<'a, *const c_char>>,
    envp: Option<NulTerminated<'a, *const c_char>>,
}

impl<'a> ExecParams<'a> {
    pub unsafe fn from_ptr(argv: *const *const c_char, envp: *const *const c_char) -> Self {
        Self {
            argv: if argv.is_null() { None } else { Some(unsafe { NulTerminated::from_ptr(argv) })},
            envp: if argv.is_null() { None } else { Some(unsafe { NulTerminated::from_ptr(envp) })},
        }
    }

    fn prepare_envp<'b>(
        &self,
        client: &Client,
        program: ThinCStr<'b>,
    ) {
        let mut program_env_buf = ArrayVec::<u8, { ENVNAME_PROGRAM.len() + 1 + PATH_MAX + 1 }>::new();
        let mut envp_buf = ArrayVec::<*const c_char, 1024>::new();
        let program = program.to_counted();
        program_env_buf
            .try_extend_from_slice(ENVNAME_PROGRAM.as_bytes())
            .unwrap();
        program_env_buf.push(b'=');
        program_env_buf
            .try_extend_from_slice(program.as_slice_with_term())
            .unwrap();

        envp_buf.clear();
        envp_buf.push(program_env_buf.as_ptr());
        envp_buf.push(client.host_path_env.data().as_ptr());
        envp_buf.push(client.ipc_fd_env.data().as_ptr());

        for env in unsafe { iter_envp(envp) } {
            if is_env_reserved(env) {
                let env_data = env.to_fat().as_slice();

                stderr_print(b"fspy: child process should not spawn with reserved env name (");
                stderr_print(env_data);
                stderr_print(b")\n");
                unsafe { libc::abort() };
            }
            envp_buf.push(env.as_ptr());
        }
        envp_buf.push(null());
    }
}
