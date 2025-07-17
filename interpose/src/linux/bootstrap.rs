use super::SYSCALL_MAGIC;
use seccompiler::{
    apply_filter, BpfProgram, SeccompAction, SeccompCondition, SeccompFilter, SeccompRule
};



pub fn bootstrap() -> seccompiler::Result<()> {
    let syscalls_with_magic_indexes: &[(i64, u8)] = &[
        (libc::SYS_readlinkat, 4),
        (libc::SYS_openat, 4),
        (libc::SYS_execve, 3),
    ];
    let filter = SeccompFilter::new(
        syscalls_with_magic_indexes
            .iter()
            .cloned()
            .map(|(syscall, magic_index)| Ok({
                (
                    syscall,
                    vec![
                        SeccompRule::new(vec![
                            SeccompCondition::new(
                                magic_index,
                                seccompiler::SeccompCmpArgLen::Qword,
                                seccompiler::SeccompCmpOp::Ne,
                                SYSCALL_MAGIC,
                            )?,
                        ])?,
                    ],
                )
            }))
            .collect::<seccompiler::Result<_>>()?,
        SeccompAction::Allow,
        SeccompAction::Trap,
        std::env::consts::ARCH.try_into()?,
    )?;
    let filter = BpfProgram::try_from(filter)?;
    apply_filter(&filter)?;
    Ok(())
}
