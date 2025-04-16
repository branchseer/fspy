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

void set_signal_handle();
void install_seccomp_filter();

void sig_handler(int signo, siginfo_t *info, void *data) {
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

void set_signal_handle(){
    struct sigaction sa;
    sigset_t sigset;

    sigfillset(&sigset);

    sa.sa_sigaction = sig_handler;
    sa.sa_mask = sigset;
    sa.sa_flags = SA_SIGINFO;

    install_seccomp_filter();

    if (sigaction(SIGSYS, &sa, NULL) == -1) {
        printf("sigaction init failed.\n");
        return ;
    }
    printf("sigaction init success.\n");
}

void install_seccomp_filter(){
    struct sock_filter filter[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, (offsetof(struct seccomp_data, nr))),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, __NR_openat, 0, 2),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, args[3])),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SECMAGIC, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_TRAP)
    };

    struct sock_fprog prog = {
        .len = (unsigned short) (sizeof(filter) / sizeof(filter[0])),
        .filter = filter,
    };

    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) == -1) {
        perror("prctl(PR_SET_NO_NEW_PRIVS)");
        abort();
    }
    if (prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &prog) == -1) {
        perror("when setting seccomp filter");
        abort();
    }
}

int main() {
    install_seccomp_filter();

    if (fork() == 0) {
        char * argv[2];
        argv[0] = "/workspaces/fspy/hello";
        argv[1] = NULL;
        char * envp[1];
        envp[0] = NULL;
        execve("/workspaces/fspy/hello", argv, envp);
    } else {
        int fd = openat(AT_FDCWD, "/etc/hosts", O_RDONLY);
        printf("fd: %d\n", fd);
    }
    // set_signal_handle();
    // if (fork() == 0) {
    // }
    // execl("/usr/bin/cat", "/usr/bin/cat", "/etc/hosts", (const char*)0);
    return 0;
}
