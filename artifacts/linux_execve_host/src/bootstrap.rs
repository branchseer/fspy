use seccompiler::{
    BpfProgram, SeccompAction, SeccompCondition, SeccompFilter, SeccompRule, apply_filter,
};

use crate::consts::SYSCALL_MAGIC;

pub fn bootstrap() {
    let filter = SeccompFilter::new(
        [
            (
                libc::SYS_openat,
                vec![
                    SeccompRule::new(vec![
                        SeccompCondition::new(
                            4,
                            seccompiler::SeccompCmpArgLen::Qword,
                            seccompiler::SeccompCmpOp::Ne,
                            SYSCALL_MAGIC,
                        )
                        .unwrap(),
                    ])
                    .unwrap(),
                ],
            ),
            #[cfg(target_arch = "x86_64")]
            (libc::SYS_mkdir, vec![]),
            #[cfg(target_arch = "x86_64")]
            (libc::SYS_open, vec![]),
        ]
        .into_iter()
        .collect(),
        SeccompAction::Allow,
        SeccompAction::Trap,
        std::env::consts::ARCH.try_into().unwrap(),
    )
    .unwrap();
    let filter = BpfProgram::try_from(filter).unwrap();
    apply_filter(&filter).unwrap();
}
