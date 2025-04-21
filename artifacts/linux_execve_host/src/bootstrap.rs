use seccompiler::{
    BpfProgram, SeccompAction, SeccompCondition, SeccompFilter, SeccompRule, apply_filter,
};

use crate::consts::SYSCALL_MAGIC;

pub fn bootstrap() {
    let syscalls_with_magic_indexes: &[(i64, u8)] = &[(libc::SYS_openat, 4), (libc::SYS_execve, 3)];
    let filter = SeccompFilter::new(
        syscalls_with_magic_indexes
            .iter()
            .cloned()
            .map(|(syscall, magic_index)| {
                (
                    syscall,
                    vec![
                        SeccompRule::new(vec![
                            SeccompCondition::new(
                                magic_index,
                                seccompiler::SeccompCmpArgLen::Qword,
                                seccompiler::SeccompCmpOp::Ne,
                                SYSCALL_MAGIC,
                            )
                            .unwrap(),
                        ])
                        .unwrap(),
                    ],
                )
            })
            .collect(),
        SeccompAction::Allow,
        SeccompAction::Trap,
        std::env::consts::ARCH.try_into().unwrap(),
    )
    .unwrap();
    let filter = BpfProgram::try_from(filter).unwrap();
    apply_filter(&filter).unwrap();
}
