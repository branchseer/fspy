use num_enum::{IntoPrimitive, TryFromPrimitive};

pub const SYSCALL_MAGIC: u64 = 0xF900d575; // 'good sys'

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum AccessKind {
    Open,
}
