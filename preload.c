#include <signal.h>
#include <stdio.h>
#include <linux/bpf_common.h>
#include <asm-generic/unistd.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <linux/prctl.h>
#include <sys/prctl.h>
#include <unistd.h>
#include <stddef.h>
#include <string.h>
#include <stdlib.h>
#include <fcntl.h>

#define SECMAGIC 0xdeadbeef


static void sig_handler(int signo, siginfo_t *info, void *data) {
    int my_signo = info->si_signo;
    // log("my_signo: %d", my_signo);
    unsigned long sysno = ((ucontext_t *) data)->uc_mcontext.regs[8];
    unsigned long arg0 = ((ucontext_t *) data)->uc_mcontext.regs[0];
    unsigned long arg1 = ((ucontext_t *) data)->uc_mcontext.regs[1];
    unsigned long arg2 = ((ucontext_t *) data)->uc_mcontext.regs[2];

    int fd;

    switch (sysno) {
        case __NR_openat:
            // syscall with args[3] SEC_MAGIC avoid infinite loop
            fd = syscall(__NR_openat, arg0, arg1, arg2, SECMAGIC);
            write(1, "opentat: ", strlen("opentat: "));
            const char * path = (const char *)arg1;
            write(1, path, strlen(path));
            write(1, "\n", 1);
            ((ucontext_t *) data)->uc_mcontext.regs[0] = fd;
            break;
        default:
            break;
    }
}

__attribute__((constructor)) static void set_signal_handle() {
    struct sigaction sa;
    sigset_t sigset;

    sigfillset(&sigset);

    sa.sa_sigaction = sig_handler;
    sa.sa_mask = sigset;
    sa.sa_flags = SA_SIGINFO;
    if (sigaction(SIGSYS, &sa, NULL) == -1) {
        printf("sigaction init failed.\n");
        return;
    }
    printf("sigaction init success.\n");
}
