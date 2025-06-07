use std::{
    convert::Infallible, env::temp_dir, ffi::c_char, fs::create_dir, io, mem, os::windows::{ffi::OsStrExt, io::AsRawHandle, process::ChildExt as _}, str::from_utf8
};

use fspy_shared::windows::FSSPY_IPC_PAYLOAD;
use futures_util::{Stream, TryStreamExt, stream::try_unfold};
use ms_detours::{DetourCopyPayloadToProcess, DetourUpdateProcessWithDll};
use tokio::{
    io::AsyncReadExt,
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
    process::{Child, Command},
};
// use detours_sys2::{DetourAttach,};

use winapi::{
    shared::minwindef::TRUE,
    um::{
        processthreadsapi::ResumeThread, securitybaseapi::AllocateLocallyUniqueId,
        winbase::CREATE_SUSPENDED, winnt::LUID,
    },
};
// use windows_sys::Win32::System::Threading::{CREATE_SUSPENDED, ResumeThread};
use winsafe::co::{CP, WC};

use crate::fixture::{Fixture, fixture};

const INTERPOSE_CDYLIB: Fixture = fixture!("fspy_interpose");

fn luid() -> io::Result<u64> {
    let mut luid = unsafe { std::mem::zeroed::<winapi::um::winnt::LUID>() };
    let ret = unsafe { winapi::um::securitybaseapi::AllocateLocallyUniqueId(&mut luid) };
    if ret == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(luid.HighPart as u32)) << 32 | u64::from(luid.LowPart))
}

fn named_pipe_server_stream(
    opts: ServerOptions,
    addr: String,
) -> io::Result<impl Stream<Item = io::Result<NamedPipeServer>>> {
    let server = opts.clone().first_pipe_instance(true).create(&addr)?;
    Ok(try_unfold(
        (opts, server, addr),
        |(opts, mut server, addr)| async move {
            server.connect().await?;
            let connected_client = server;
            server = opts.create(&addr)?;
            io::Result::Ok(Some((connected_client, (opts, server, addr))))
        },
    ))
}

pub fn spawn(mut command: Command) -> io::Result<(Child, impl Future<Output = io::Result<()>>)> {
    let tmp_dir = temp_dir().join("fspy");
    let _ = create_dir(&tmp_dir);
    let interpose_cdylib = INTERPOSE_CDYLIB.write_to(&tmp_dir, ".dll").unwrap();

    let interpose_cdylib = interpose_cdylib
        .as_os_str()
        .encode_wide()
        .collect::<Vec<u16>>();
    let mut interpose_cdylib =
        winsafe::WideCharToMultiByte(CP::ACP, WC::NoValue, &interpose_cdylib, None, None)
            .map_err(|err| io::Error::from_raw_os_error(err.raw() as i32))?;

    interpose_cdylib.push(0);

    command.creation_flags(CREATE_SUSPENDED);

    let pipe_name = format!(r"\\.\pipe\fspy_ipc_{:x}", luid()?);

    let pipe_server_opts = {
        let mut opts = ServerOptions::new();
        opts.pipe_mode(PipeMode::Message);
        opts.access_outbound(false);
        opts
    };

    let server_stream = named_pipe_server_stream(pipe_server_opts, pipe_name.clone().into())?;

    let fut = server_stream.try_for_each_concurrent(None, |mut connection| async move {
        let mut buf = [0u8; 4097];
        loop {
            let n = connection.read(&mut buf).await?;
            if n == 0 {
                break io::Result::Ok(());
            }
            let msg = &buf[..n];
            eprintln!("{:?}", n);
        }
    });
    let child = command.spawn_with(|std_command| {
        let std_child = std_command.spawn()?;

        let mut interpose_cdylib = interpose_cdylib.as_ptr().cast::<c_char>();
        let process_handle = std_child.as_raw_handle().cast::<winapi::ctypes::c_void>();
        let success =
            unsafe { DetourUpdateProcessWithDll(process_handle, &mut interpose_cdylib, 1) };
        if success != TRUE {
            return Err(io::Error::last_os_error());
        }
        let success = unsafe {
            DetourCopyPayloadToProcess(
                process_handle,
                &FSSPY_IPC_PAYLOAD,
                pipe_name.as_ptr().cast(),
                pipe_name.len().try_into().unwrap(),
            )
        };
        if success != TRUE {
            return Err(io::Error::last_os_error());
        }

        let main_thread_handle = std_child.main_thread_handle();
        let resume_thread_ret =
            unsafe { ResumeThread(main_thread_handle.as_raw_handle().cast()) } as i32;

        if resume_thread_ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(std_child)
    })?;
    Ok((child, fut))
}
