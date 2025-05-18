use std::{iter::once, ptr::null};

use arrayvec::ArrayVec;
use libc::c_char;

use crate::{abort::abort_with, client::Client, consts::ENVNAME_PROGRAM, is_env_reserved, nul::NulTerminated};

pub struct ExecContext<'a> {
    program: NulTerminated<'a, u8>,
    argv: Option<NulTerminated<'a, *const u8>>,
    envp: Option<NulTerminated<'a, *const u8>>,
    cstr_buf: ArrayVec<u8, 1024>,
    ptr_buf: ArrayVec<*const u8, 1024>,
    client: &'a Client<'a>
}

pub fn exec_with_host(program: *const c_char, argv: *const *const c_char, envp: *const *const c_char, client: &Client<'_>) -> nix::Result<()> {
    let program = unsafe { NulTerminated::from_ptr(program.cast()) };
    let argv =  unsafe { NulTerminated::from_nullable_ptr(argv.cast()) };
    let envp =  unsafe { NulTerminated::from_nullable_ptr(envp.cast()) };

    // taking mut reference to ensure ctx not to be moved which would invalidate new envp/argv 
    let ctx = &mut ExecContext {
        program,
        argv,envp, cstr_buf: ArrayVec::new(), ptr_buf: ArrayVec::new(), client
    };
    Ok(())
}

impl<'a> ExecContext<'a> {
    fn copy_cstr(&mut self, content: impl Iterator<Item = u8>) -> nix::Result<NulTerminated<'_, u8>> {
        let start = self.cstr_buf.len();
        for byte in content {
            self.cstr_buf.try_push(byte).map_err(|_| nix::Error::E2BIG)?;
        }
        self.cstr_buf.try_push(0).map_err(|_| nix::Error::E2BIG)?;
        Ok(unsafe { NulTerminated::from_ptr(&self.cstr_buf[start]) })
    }
    fn try_push_ptr(&mut self, ptr: *const u8) -> nix::Result<()> {
        self.ptr_buf.try_push(ptr).map_err(|_| nix::Error::E2BIG)
    }

    fn prepare_envp(
        &mut self,
    ) -> nix::Result<*const *const c_char> {
        let program_env_ptr = self.copy_cstr(
            ENVNAME_PROGRAM.as_bytes().into_iter().copied().chain(once(b'=')).chain(self.program.copied())
        )?.as_ptr();

        let envp_start = self.ptr_buf.len();
        self.try_push_ptr(program_env_ptr)?;
        self.try_push_ptr(self.client.host_path_env.data().as_ptr());
        self.try_push_ptr(self.client.ipc_fd_env.data().as_ptr());

        if let Some(envp) = self.envp {
            let envs = envp.copied().map(|env_ptr| unsafe { NulTerminated::from_ptr(env_ptr) });
            for env in envs {
                if is_env_reserved(env) {
                    let env_data = env.to_counted().as_slice();
                    abort_with(&["child process should not spawn with reserved env name (".as_bytes(), env_data, b")"])
                }
                self.try_push_ptr(env.as_ptr())?;
            }
        }
        self.try_push_ptr(null())?;
        let envp: *const *const u8 = &self.ptr_buf[envp_start];
        Ok(envp.cast())
    }

    fn prepare_argv(&mut self) -> nix::Result<*const *const c_char> {
        // parse_shebang_recursive<DEFAULT_PEEK_SIZE, _, _, _>(Default::default(), reader, open, on_arg_reverse)
        Ok(null())
    }
}
