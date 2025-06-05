use windows_sys::Win32::Foundation::{BOOL, HINSTANCE};
use ms_detours::DetourIsHelperProcess;

#[unsafe(no_mangle)]
#[allow(non_snake_case, unused_variables)]
extern "system" fn DllMain(hinstance: HINSTANCE, reason: u32, _: *mut std::ffi::c_void) -> BOOL {
    if unsafe { DetourIsHelperProcess() } != 0 {
        return 1;
    }
    eprintln!("Dllmain");
    1
}
