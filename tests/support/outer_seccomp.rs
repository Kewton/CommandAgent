//! A real kernel refusal of the first step of the Linux strict isolation (Issue #982), for the
//! missing-capability controls: a process started under this seccomp filter, and everything it
//! starts, gets `EPERM` from `unshare`, so the strict probe stops there. Test code only:
//! production has no switch that asks for this.

use std::os::unix::process::CommandExt as _;
use std::process::Command;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000_003E;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC000_00B7;

const LOAD: u16 = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
const EQUAL: u16 = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
const RETURN: u16 = (libc::BPF_RET | libc::BPF_K) as u16;

/// `program`, started under a filter that answers `EPERM` to `unshare` on this architecture
/// and allows every other call.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    // SAFETY: `install` makes two system calls on a program on its own stack; it allocates
    // nothing and takes no lock, so it is async-signal-safe.
    unsafe {
        command.pre_exec(install);
    }
    command
}

fn install() -> std::io::Result<()> {
    let refuse = libc::SECCOMP_RET_ERRNO | libc::EPERM as u32;
    let mut filter = [
        libc::sock_filter {
            code: LOAD,
            jt: 0,
            jf: 0,
            k: 4,
        },
        libc::sock_filter {
            code: EQUAL,
            jt: 0,
            jf: 3,
            k: AUDIT_ARCH,
        },
        libc::sock_filter {
            code: LOAD,
            jt: 0,
            jf: 0,
            k: 0,
        },
        libc::sock_filter {
            code: EQUAL,
            jt: 0,
            jf: 1,
            k: libc::SYS_unshare as u32,
        },
        libc::sock_filter {
            code: RETURN,
            jt: 0,
            jf: 0,
            k: refuse,
        },
        libc::sock_filter {
            code: RETURN,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ALLOW,
        },
    ];
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    let (on, unused): (libc::c_ulong, libc::c_ulong) = (1, 0);
    // SAFETY: `prctl` takes no pointer here, and `seccomp` reads `program`, which outlives it.
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, on, unused, unused, unused) != 0
            || libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_SET_MODE_FILTER,
                0u32,
                &raw const program,
            ) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
