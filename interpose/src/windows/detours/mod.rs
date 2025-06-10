
mod create_process;
mod fileapi;
mod nt;

use super::detour::DetourAny;
use constcat::concat_slices;

pub const DETOURS: &[DetourAny] = concat_slices!([DetourAny]:
    create_process::DETOURS,
    fileapi::DETOURS,
);
