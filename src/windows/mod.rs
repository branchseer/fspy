use std::{
    convert::Infallible,
    env::temp_dir,
    ffi::{c_char, c_void},
    fs::{File, OpenOptions, create_dir},
    io, mem,
    os::windows::{ffi::OsStrExt, io::AsRawHandle, process::ChildExt as _},
    ptr::{null, null_mut},
    str::from_utf8,
};

use bincode::borrow_decode_from_slice;
use fspy_shared::{
    ipc::{BINCODE_CONFIG, PathAccess},
    windows::{PAYLOAD_ID, Payload},
};
use futures_util::{
    Stream, TryStreamExt,
    future::{join, try_join},
    stream::try_unfold,
};
use ms_detours::{DetourCopyPayloadToProcess, DetourUpdateProcessWithDll};
use tokio::{
    io::AsyncReadExt,
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};
// use detours_sys2::{DetourAttach,};

use winapi::{
    shared::minwindef::{FALSE, TRUE},
    um::{
        handleapi::DuplicateHandle,
        processthreadsapi::{GetCurrentProcess, ResumeThread},
        winbase::CREATE_SUSPENDED,
        winnt::{DUPLICATE_SAME_ACCESS, GENERIC_WRITE},
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

pub async fn spawn(
    mut command: Command,
) -> io::Result<(Child, impl Future<Output = io::Result<()>>)> {
    let tmp_dir = temp_dir().join("fspy");
    let _ = create_dir(&tmp_dir);
    let dll_path = INTERPOSE_CDYLIB.write_to(&tmp_dir, ".dll").unwrap();

    let wide_dll_path = dll_path.as_os_str().encode_wide().collect::<Vec<u16>>();
    let mut asni_dll_path =
        winsafe::WideCharToMultiByte(CP::ACP, WC::NoValue, &wide_dll_path, None, None)
            .map_err(|err| io::Error::from_raw_os_error(err.raw() as i32))?;

    asni_dll_path.push(0);

    let asni_dll_path_with_nul = asni_dll_path.as_slice();

    command.creation_flags(CREATE_SUSPENDED);

    let pipe_name = format!(r"\\.\pipe\fspy_ipc_{:x}", luid()?);

    // let pipe_server_opts = {
    //     let mut opts = ServerOptions::new();
    //     opts.pipe_mode(PipeMode::Message);
    //     opts.access_outbound(false);
    //     opts
    // };
    let mut pipe_receiver = ServerOptions::new()
        .pipe_mode(PipeMode::Message)
        .access_outbound(false)
        .access_inbound(true)
        .create(&pipe_name)?;

    let connect_fut = pipe_receiver.connect();

    let pipe_sender = OpenOptions::new().write(true).open(&pipe_name).unwrap();

    connect_fut.await?;

    let fut = async move {
        const MESSAGE_MAX_LEN: usize = 4096;
        let mut buf = vec![0u8; MESSAGE_MAX_LEN];
        loop {
            let n = pipe_receiver.read(&mut buf).await?;
            if n == 0 {
                break io::Result::Ok(());
            }
            let msg = &buf[..n];
            let (path_access, decoded_len) =
                borrow_decode_from_slice::<'_, PathAccess, _>(msg, BINCODE_CONFIG).unwrap();
            assert_eq!(decoded_len, msg.len());
            // eprintln!("{:?}", path_access);
        }
    };

    let child = command.spawn_with(|std_command| {
        let std_child = std_command.spawn()?;

        let mut dll_paths = asni_dll_path_with_nul.as_ptr().cast::<c_char>();
        let process_handle = std_child.as_raw_handle().cast::<winapi::ctypes::c_void>();
        let success = unsafe { DetourUpdateProcessWithDll(process_handle, &mut dll_paths, 1) };
        if success != TRUE {
            return Err(io::Error::last_os_error());
        }

        let mut handle_in_child: *mut c_void = null_mut();
        let ret = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                pipe_sender.as_raw_handle(),
                process_handle,
                &mut handle_in_child,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if ret == 0 {
            return Err(io::Error::last_os_error());
        }

        let payload = Payload {
            pipe_handle: handle_in_child.addr(),
            asni_dll_path_with_nul,
        };
        let payload_bytes = bincode::encode_to_vec(payload, BINCODE_CONFIG).unwrap();
        let success = unsafe {
            DetourCopyPayloadToProcess(
                process_handle,
                &PAYLOAD_ID,
                payload_bytes.as_ptr().cast(),
                payload_bytes.len().try_into().unwrap(),
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

    drop(pipe_sender);
    Ok((child, fut))
}
