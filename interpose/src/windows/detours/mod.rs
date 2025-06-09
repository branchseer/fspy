use super::detour::DetourAny;

mod create_process;

pub const DETOURS: &[DetourAny] = create_process::DETOURS;
