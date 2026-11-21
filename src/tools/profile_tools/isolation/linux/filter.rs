//! The seccomp filter of the strict launch on Linux (Issue #982): classic BPF written by hand,
//! with nothing beyond the `libc` crate, for x86_64 and aarch64.
//!
//! The architecture is read first. Any other ABI, i386 or x32 on x86_64 and AArch32 on aarch64,
//! is killed before a number is read. The listed calls are refused with an errno, or judged by
//! their arguments, and every other call up to the highest number the table names is allowed:
//! the private root and Landlock already bound what a call can reach, and this closes what they
//! cannot, the network, new processes, other processes, and the mount tree. A call that names a
//! process by its pid reaches only the tool itself: 0 or its own pid. A number above the table's
//! highest is newer than this table and answers `ENOSYS`, so the C library falls back to an
//! older call that is judged here. The table stays a list of refusals, not of permissions.
//!
//! Each number in the tables is compared with the `libc` crate at compile time for the
//! architecture being built, and the tests run the generated program through an interpreter of
//! their own against a table typed apart from this one.

/// What the filter does with one call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    /// `EPERM`.
    Refuse,
    /// `ENOSYS`: `clone3`, so that the C library falls back to `clone`, which is judged here.
    NotImplemented,
    /// `clone` with `CLONE_THREAD` and without any namespace flag: a thread, never a process.
    ThreadOnly,
    /// A signal call whose first argument is the tool's own process.
    SelfOnly,
    /// A call whose first argument is a pid: 0 or the tool's own pid only.
    PidFirst,
    /// `setpriority` and `getpriority`: `PRIO_PROCESS` and 0 or the tool's own pid only.
    PriorityOfItself,
    /// `ioprio_set` and `ioprio_get`: `IOPRIO_WHO_PROCESS` and 0 or the tool's own pid only.
    IoPriorityOfItself,
    /// `fcntl`: `F_SETOWN` only to 0 or the tool's own pid, `F_SETOWN_EX` refused (its owner is
    /// behind a pointer), every other command allowed.
    OwnerItself,
    /// `setpgid(0, 0)` only: the tool may lead its own group and join no other.
    OwnGroupOnly,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) name: &'static str,
    /// `None` where the call does not exist on this architecture.
    pub(super) number: Option<u32>,
    pub(super) action: Action,
}

#[derive(Debug)]
pub(super) struct Arch {
    /// For the tests' messages.
    #[cfg(test)]
    pub(super) name: &'static str,
    pub(super) audit_arch: u32,
    /// x86_64 only: a number with this bit set is a call of the x32 ABI.
    pub(super) x32_bit: Option<u32>,
    pub(super) entries: &'static [Entry],
}

pub(super) const AUDIT_ARCH_X86_64: u32 = 0xC000_003E;
pub(super) const AUDIT_ARCH_AARCH64: u32 = 0xC000_00B7;
pub(super) const X32_SYSCALL_BIT: u32 = 0x4000_0000;

const CLONE_THREAD: u32 = 0x0001_0000;
/// `CLONE_NEWNS`, `CLONE_NEWCGROUP`, `CLONE_NEWUTS`, `CLONE_NEWIPC`, `CLONE_NEWUSER`,
/// `CLONE_NEWPID`, and `CLONE_NEWNET`. `CLONE_NEWTIME` lies inside the signal byte of `clone`
/// and is reached only through `clone3` and `unshare`, both refused.
const CLONE_NAMESPACES: u32 =
    0x0002_0000 | 0x0200_0000 | 0x0400_0000 | 0x0800_0000 | 0x1000_0000 | 0x2000_0000 | 0x4000_0000;

macro_rules! tables {
    ($( $name:ident : $x86_64:expr, $aarch64:expr, $action:ident ; )*) => {
        const X86_64_ENTRIES: &[Entry] = &[
            $( Entry { name: stringify!($name), number: $x86_64, action: Action::$action } ),*
        ];
        const AARCH64_ENTRIES: &[Entry] = &[
            $( Entry { name: stringify!($name), number: $aarch64, action: Action::$action } ),*
        ];
    };
}

// name: x86_64, aarch64, action. `socketcall` exists only in the i386 and AArch32 ABIs, which
// the architecture check kills, so it has no row.
tables! {
    // The network: every socket is refused at its creation, and every use of one.
    socket: Some(41), Some(198), Refuse;
    socketpair: Some(53), Some(199), Refuse;
    connect: Some(42), Some(203), Refuse;
    bind: Some(49), Some(200), Refuse;
    listen: Some(50), Some(201), Refuse;
    accept: Some(43), Some(202), Refuse;
    accept4: Some(288), Some(242), Refuse;
    sendto: Some(44), Some(206), Refuse;
    recvfrom: Some(45), Some(207), Refuse;
    sendmsg: Some(46), Some(211), Refuse;
    recvmsg: Some(47), Some(212), Refuse;
    sendmmsg: Some(307), Some(269), Refuse;
    recvmmsg: Some(299), Some(243), Refuse;
    shutdown: Some(48), Some(210), Refuse;
    io_uring_setup: Some(425), Some(425), Refuse;
    io_uring_enter: Some(426), Some(426), Refuse;
    io_uring_register: Some(427), Some(427), Refuse;
    // New processes.
    fork: Some(57), None, Refuse;
    vfork: Some(58), None, Refuse;
    clone: Some(56), Some(220), ThreadOnly;
    clone3: Some(435), Some(435), NotImplemented;
    // An exec other than of a path, which Landlock judges.
    execveat: Some(322), Some(281), Refuse;
    memfd_create: Some(319), Some(279), Refuse;
    memfd_secret: Some(447), Some(447), Refuse;
    // Other processes.
    ptrace: Some(101), Some(117), Refuse;
    process_vm_readv: Some(310), Some(270), Refuse;
    process_vm_writev: Some(311), Some(271), Refuse;
    pidfd_open: Some(434), Some(434), Refuse;
    pidfd_getfd: Some(438), Some(438), Refuse;
    pidfd_send_signal: Some(424), Some(424), Refuse;
    process_madvise: Some(440), Some(440), Refuse;
    process_mrelease: Some(448), Some(448), Refuse;
    kcmp: Some(312), Some(272), Refuse;
    // Signals, to the tool itself only.
    kill: Some(62), Some(129), SelfOnly;
    tgkill: Some(234), Some(131), SelfOnly;
    rt_sigqueueinfo: Some(129), Some(138), SelfOnly;
    rt_tgsigqueueinfo: Some(297), Some(240), SelfOnly;
    tkill: Some(200), Some(130), Refuse;
    // Namespaces, the root, and the mount tree, once they are set up.
    unshare: Some(272), Some(97), Refuse;
    setns: Some(308), Some(268), Refuse;
    mount: Some(165), Some(40), Refuse;
    umount2: Some(166), Some(39), Refuse;
    pivot_root: Some(155), Some(41), Refuse;
    chroot: Some(161), Some(51), Refuse;
    open_tree: Some(428), Some(428), Refuse;
    move_mount: Some(429), Some(429), Refuse;
    fsopen: Some(430), Some(430), Refuse;
    fsconfig: Some(431), Some(431), Refuse;
    fsmount: Some(432), Some(432), Refuse;
    fspick: Some(433), Some(433), Refuse;
    mount_setattr: Some(442), Some(442), Refuse;
    // Not in `libc` 0.2.186, so the compile-time check below does not cover it; the tests do.
    open_tree_attr: Some(467), Some(467), Refuse;
    // Kernel objects and file handles.
    bpf: Some(321), Some(280), Refuse;
    userfaultfd: Some(323), Some(282), Refuse;
    perf_event_open: Some(298), Some(241), Refuse;
    keyctl: Some(250), Some(219), Refuse;
    add_key: Some(248), Some(217), Refuse;
    request_key: Some(249), Some(218), Refuse;
    open_by_handle_at: Some(304), Some(265), Refuse;
    name_to_handle_at: Some(303), Some(264), Refuse;
    fanotify_init: Some(300), Some(262), Refuse;
    fanotify_mark: Some(301), Some(263), Refuse;
    // Other processes by their pid: the kernel judges these by uid alone, and the tool keeps the
    // host's uid, so only 0 or the tool's own pid is let through. Another thread of the tool,
    // named by its tid, is refused too.
    prlimit64: Some(302), Some(261), PidFirst;
    sched_setaffinity: Some(203), Some(122), PidFirst;
    sched_getaffinity: Some(204), Some(123), PidFirst;
    sched_setscheduler: Some(144), Some(119), PidFirst;
    sched_getscheduler: Some(145), Some(120), PidFirst;
    sched_setparam: Some(142), Some(118), PidFirst;
    sched_getparam: Some(143), Some(121), PidFirst;
    sched_setattr: Some(314), Some(274), PidFirst;
    sched_getattr: Some(315), Some(275), PidFirst;
    sched_rr_get_interval: Some(148), Some(127), PidFirst;
    getpgid: Some(121), Some(155), PidFirst;
    getsid: Some(124), Some(156), PidFirst;
    get_robust_list: Some(274), Some(100), PidFirst;
    migrate_pages: Some(256), Some(238), PidFirst;
    move_pages: Some(279), Some(239), PidFirst;
    setpriority: Some(141), Some(140), PriorityOfItself;
    getpriority: Some(140), Some(141), PriorityOfItself;
    ioprio_set: Some(251), Some(30), IoPriorityOfItself;
    ioprio_get: Some(252), Some(31), IoPriorityOfItself;
    // `F_SETOWN` with `O_ASYNC` and `F_SETSIG` would send any signal without `kill`.
    fcntl: Some(72), Some(25), OwnerItself;
    setpgid: Some(109), Some(154), OwnGroupOnly;
    // Its pid is behind a pointer, which the filter cannot read.
    capget: Some(125), Some(90), Refuse;
    syslog: Some(103), Some(116), Refuse;
}

pub(super) const X86_64: Arch = Arch {
    #[cfg(test)]
    name: "x86_64",
    audit_arch: AUDIT_ARCH_X86_64,
    x32_bit: Some(X32_SYSCALL_BIT),
    entries: X86_64_ENTRIES,
};

pub(super) const AARCH64: Arch = Arch {
    #[cfg(test)]
    name: "aarch64",
    audit_arch: AUDIT_ARCH_AARCH64,
    x32_bit: None,
    entries: AARCH64_ENTRIES,
};

/// The table of the architecture this binary runs on. Any other is unavailable.
pub(super) fn native() -> Option<&'static Arch> {
    if cfg!(target_arch = "x86_64") {
        Some(&X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Some(&AARCH64)
    } else {
        None
    }
}

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut index = 0;
    while index < a.len() {
        if a[index] != b[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The number of `name` in `table`. A name the table lacks stops the build.
const fn number_of(table: &[Entry], name: &str) -> u32 {
    let mut index = 0;
    while index < table.len() {
        if same(table[index].name, name) {
            match table[index].number {
                Some(number) => return number,
                None => panic!("the call has no number on this architecture"),
            }
        }
        index += 1;
    }
    panic!("the call is missing from the table")
}

/// Each number of the table being built is the `libc` crate's own, or the build stops. Building
/// for x86_64 and for aarch64 checks both tables.
macro_rules! agrees_with_libc {
    ($table:expr; $( $name:ident => $constant:ident ),* $(,)?) => {
        const _: () = {
            $( assert!(number_of($table, stringify!($name)) as libc::c_long == libc::$constant); )*
        };
    };
}

macro_rules! agrees_on_both {
    ($table:expr) => {
        agrees_with_libc!($table;
            socket => SYS_socket, socketpair => SYS_socketpair, connect => SYS_connect,
            bind => SYS_bind, listen => SYS_listen, accept => SYS_accept, accept4 => SYS_accept4,
            sendto => SYS_sendto, recvfrom => SYS_recvfrom, sendmsg => SYS_sendmsg,
            recvmsg => SYS_recvmsg, sendmmsg => SYS_sendmmsg, recvmmsg => SYS_recvmmsg,
            shutdown => SYS_shutdown, io_uring_setup => SYS_io_uring_setup,
            io_uring_enter => SYS_io_uring_enter, io_uring_register => SYS_io_uring_register,
            clone => SYS_clone, clone3 => SYS_clone3, execveat => SYS_execveat,
            memfd_create => SYS_memfd_create, memfd_secret => SYS_memfd_secret,
            ptrace => SYS_ptrace, process_vm_readv => SYS_process_vm_readv,
            process_vm_writev => SYS_process_vm_writev, pidfd_open => SYS_pidfd_open,
            pidfd_getfd => SYS_pidfd_getfd, pidfd_send_signal => SYS_pidfd_send_signal,
            process_madvise => SYS_process_madvise, process_mrelease => SYS_process_mrelease,
            kcmp => SYS_kcmp, kill => SYS_kill, tgkill => SYS_tgkill,
            rt_sigqueueinfo => SYS_rt_sigqueueinfo, rt_tgsigqueueinfo => SYS_rt_tgsigqueueinfo,
            tkill => SYS_tkill, unshare => SYS_unshare, setns => SYS_setns, mount => SYS_mount,
            umount2 => SYS_umount2, pivot_root => SYS_pivot_root, chroot => SYS_chroot,
            open_tree => SYS_open_tree, move_mount => SYS_move_mount, fsopen => SYS_fsopen,
            fsconfig => SYS_fsconfig, fsmount => SYS_fsmount, fspick => SYS_fspick,
            mount_setattr => SYS_mount_setattr, bpf => SYS_bpf, userfaultfd => SYS_userfaultfd,
            perf_event_open => SYS_perf_event_open, keyctl => SYS_keyctl, add_key => SYS_add_key,
            request_key => SYS_request_key, open_by_handle_at => SYS_open_by_handle_at,
            name_to_handle_at => SYS_name_to_handle_at, fanotify_init => SYS_fanotify_init,
            fanotify_mark => SYS_fanotify_mark, prlimit64 => SYS_prlimit64,
            sched_setaffinity => SYS_sched_setaffinity,
            sched_getaffinity => SYS_sched_getaffinity,
            sched_setscheduler => SYS_sched_setscheduler,
            sched_getscheduler => SYS_sched_getscheduler,
            sched_setparam => SYS_sched_setparam, sched_getparam => SYS_sched_getparam,
            sched_setattr => SYS_sched_setattr, sched_getattr => SYS_sched_getattr,
            sched_rr_get_interval => SYS_sched_rr_get_interval, getpgid => SYS_getpgid,
            getsid => SYS_getsid, get_robust_list => SYS_get_robust_list,
            migrate_pages => SYS_migrate_pages, move_pages => SYS_move_pages,
            setpriority => SYS_setpriority, getpriority => SYS_getpriority,
            ioprio_set => SYS_ioprio_set, ioprio_get => SYS_ioprio_get, fcntl => SYS_fcntl,
            setpgid => SYS_setpgid, capget => SYS_capget, syslog => SYS_syslog,
        );
    };
}

#[cfg(target_arch = "x86_64")]
agrees_on_both!(X86_64_ENTRIES);
#[cfg(target_arch = "x86_64")]
agrees_with_libc!(X86_64_ENTRIES; fork => SYS_fork, vfork => SYS_vfork);
#[cfg(target_arch = "aarch64")]
agrees_on_both!(AARCH64_ENTRIES);

const LOAD_WORD: u16 = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
const JUMP_EQUAL: u16 = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
const JUMP_AT_LEAST: u16 = (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16;
const JUMP_ANY_BIT: u16 = (libc::BPF_JMP | libc::BPF_JSET | libc::BPF_K) as u16;
const RETURN: u16 = (libc::BPF_RET | libc::BPF_K) as u16;
/// Offsets in `struct seccomp_data`. Both architectures are little endian, so the low half of
/// the first argument comes first.
const NUMBER_AT: u32 = 0;
const ARCH_AT: u32 = 4;
const FIRST_ARGUMENT_LOW_AT: u32 = 16;
/// The low half of argument `index`: each argument is 8 bytes. The kernel reads a pid, a
/// `which`, and an `fcntl` command or owner as an `int`, so the low half is the whole of it.
const fn argument_low_at(index: u32) -> u32 {
    FIRST_ARGUMENT_LOW_AT + 8 * index
}
const PRIO_PROCESS: u32 = 0;
const IOPRIO_WHO_PROCESS: u32 = 1;
const F_SETOWN: u32 = 8;
const F_SETOWN_EX: u32 = 15;
const _: () = assert!(F_SETOWN as libc::c_int == libc::F_SETOWN);
pub(super) const KILL_PROCESS: u32 = libc::SECCOMP_RET_KILL_PROCESS;
pub(super) const ALLOW: u32 = libc::SECCOMP_RET_ALLOW;
pub(super) const ERRNO: u32 = libc::SECCOMP_RET_ERRNO;
/// No process has this pid: the child writes its own over it.
const PID_PLACEHOLDER: u32 = 0;

const fn statement(code: u16, k: u32) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

const fn jump(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

/// A program, with the places where the child writes its own pid before it installs it.
pub(super) struct Program {
    pub(super) instructions: Vec<libc::sock_filter>,
    pub(super) pid_slots: Vec<usize>,
}

pub(super) fn build(arch: &Arch) -> Program {
    let refuse = ERRNO | libc::EPERM as u32;
    let mut program = vec![
        statement(LOAD_WORD, ARCH_AT),
        jump(JUMP_EQUAL, arch.audit_arch, 1, 0),
        statement(RETURN, KILL_PROCESS),
        statement(LOAD_WORD, NUMBER_AT),
    ];
    if let Some(bit) = arch.x32_bit {
        program.push(jump(JUMP_AT_LEAST, bit, 0, 1));
        program.push(statement(RETURN, KILL_PROCESS));
    }
    let mut pid_slots = Vec::new();
    // One block per row. A block returns, or falls through with the number still loaded.
    for entry in arch.entries {
        let Some(number) = entry.number else {
            continue;
        };
        match entry.action {
            Action::Refuse => {
                program.push(jump(JUMP_EQUAL, number, 0, 1));
                program.push(statement(RETURN, refuse));
            }
            Action::NotImplemented => {
                program.push(jump(JUMP_EQUAL, number, 0, 1));
                program.push(statement(RETURN, ERRNO | libc::ENOSYS as u32));
            }
            Action::ThreadOnly => {
                // The kernel reads only the low 32 bits of the flags of `clone`.
                program.push(jump(JUMP_EQUAL, number, 0, 5));
                program.push(statement(LOAD_WORD, FIRST_ARGUMENT_LOW_AT));
                program.push(jump(JUMP_ANY_BIT, CLONE_THREAD, 0, 2));
                program.push(jump(JUMP_ANY_BIT, CLONE_NAMESPACES, 1, 0));
                program.push(statement(RETURN, ALLOW));
                program.push(statement(RETURN, refuse));
            }
            Action::SelfOnly => {
                // A pid is an `int`: the kernel reads only the low 32 bits.
                program.push(jump(JUMP_EQUAL, number, 0, 4));
                program.push(statement(LOAD_WORD, FIRST_ARGUMENT_LOW_AT));
                pid_slots.push(program.len());
                program.push(jump(JUMP_EQUAL, PID_PLACEHOLDER, 0, 1));
                program.push(statement(RETURN, ALLOW));
                program.push(statement(RETURN, refuse));
            }
            Action::PidFirst => {
                let mut body = Block::default();
                body.load(0);
                body.zero_or_itself(refuse);
                body.guard(&mut program, &mut pid_slots, number);
            }
            Action::PriorityOfItself | Action::IoPriorityOfItself => {
                let which = if entry.action == Action::PriorityOfItself {
                    PRIO_PROCESS
                } else {
                    IOPRIO_WHO_PROCESS
                };
                let mut body = Block::default();
                body.load(0);
                // Another `which` goes to the refusal, past the load, the two checks of the pid.
                body.push(jump(JUMP_EQUAL, which, 0, 3));
                body.load(1);
                body.zero_or_itself(refuse);
                body.guard(&mut program, &mut pid_slots, number);
            }
            Action::OwnerItself => {
                let mut body = Block::default();
                body.load(1);
                // `F_SETOWN_EX` goes to the refusal; any command but `F_SETOWN` to the allow.
                body.push(jump(JUMP_EQUAL, F_SETOWN_EX, 4, 0));
                body.push(jump(JUMP_EQUAL, F_SETOWN, 0, 4));
                body.load(2);
                body.zero_or_itself(refuse);
                body.guard(&mut program, &mut pid_slots, number);
            }
            Action::OwnGroupOnly => {
                let mut body = Block::default();
                body.load(0);
                body.push(jump(JUMP_EQUAL, 0, 0, 2));
                body.load(1);
                body.push(jump(JUMP_EQUAL, 0, 1, 0));
                body.push(statement(RETURN, refuse));
                body.push(statement(RETURN, ALLOW));
                body.guard(&mut program, &mut pid_slots, number);
            }
        }
    }
    // Above the highest number the table names: a call newer than this table.
    program.push(jump(JUMP_AT_LEAST, ceiling(arch) + 1, 0, 1));
    program.push(statement(RETURN, ERRNO | libc::ENOSYS as u32));
    program.push(statement(RETURN, ALLOW));
    Program {
        instructions: program,
        pid_slots,
    }
}

/// The highest number the table of `arch` names.
pub(super) fn ceiling(arch: &Arch) -> u32 {
    arch.entries
        .iter()
        .filter_map(|entry| entry.number)
        .max()
        .unwrap_or(0)
}

/// The body of a block judged by its arguments. Every body returns on each of its paths, so the
/// next block never starts with an argument loaded.
#[derive(Default)]
struct Block {
    instructions: Vec<libc::sock_filter>,
    pid_slots: Vec<usize>,
}

impl Block {
    fn push(&mut self, instruction: libc::sock_filter) {
        self.instructions.push(instruction);
    }

    fn load(&mut self, argument: u32) {
        self.push(statement(LOAD_WORD, argument_low_at(argument)));
    }

    /// The loaded value is 0 or the tool's own pid, or the call is refused.
    fn zero_or_itself(&mut self, refuse: u32) {
        self.push(jump(JUMP_EQUAL, 0, 2, 0));
        self.pid_slots.push(self.instructions.len());
        self.push(jump(JUMP_EQUAL, PID_PLACEHOLDER, 1, 0));
        self.push(statement(RETURN, refuse));
        self.push(statement(RETURN, ALLOW));
    }

    /// The body, entered only for `number`. Every body here is under ten instructions, so the
    /// skip fits a jump.
    fn guard(self, program: &mut Vec<libc::sock_filter>, pid_slots: &mut Vec<usize>, number: u32) {
        let skip = u8::try_from(self.instructions.len()).unwrap_or(u8::MAX);
        program.push(jump(JUMP_EQUAL, number, 0, skip));
        let start = program.len();
        pid_slots.extend(self.pid_slots.iter().map(|slot| start + slot));
        program.extend(self.instructions);
    }
}

#[cfg(test)]
mod tests {
    //! The generated program, judged by an interpreter written here and a table typed here from
    //! the kernel's own syscall tables, apart from the table above.

    use super::{
        AARCH64, ALLOW, AUDIT_ARCH_AARCH64, AUDIT_ARCH_X86_64, Arch, ERRNO, KILL_PROCESS,
        X32_SYSCALL_BIT, X86_64, build,
    };

    const AUDIT_ARCH_I386: u32 = 0x4000_0003;
    const AUDIT_ARCH_ARM: u32 = 0x4000_0028;
    const EPERM: u32 = ERRNO | 1;
    const ENOSYS: u32 = ERRNO | 38;

    /// One classic BPF program run over a `struct seccomp_data`, little endian, with the first
    /// arguments given and the rest 0.
    fn run(
        program: &[libc::sock_filter],
        number: u32,
        arch: u32,
        arguments: &[u64],
    ) -> Result<u32, String> {
        let mut data = [0u8; 64];
        data[0..4].copy_from_slice(&number.to_le_bytes());
        data[4..8].copy_from_slice(&arch.to_le_bytes());
        for (index, argument) in arguments.iter().enumerate().take(6) {
            let at = 16 + 8 * index;
            data[at..at + 8].copy_from_slice(&argument.to_le_bytes());
        }
        let mut accumulator = 0u32;
        let mut at = 0usize;
        for _ in 0..program.len() + 1 {
            let Some(instruction) = program.get(at) else {
                return Err(format!("ran past the end at {at}"));
            };
            match u32::from(instruction.code) {
                code if code == libc::BPF_LD | libc::BPF_W | libc::BPF_ABS => {
                    let offset = instruction.k as usize;
                    let word = data
                        .get(offset..offset + 4)
                        .filter(|_| offset.is_multiple_of(4))
                        .ok_or_else(|| format!("a load at {offset}"))?;
                    accumulator = u32::from_le_bytes(word.try_into().unwrap());
                    at += 1;
                }
                code if code & 0x07 == libc::BPF_JMP => {
                    let taken = match code & 0xf0 {
                        operation if operation == libc::BPF_JEQ => accumulator == instruction.k,
                        operation if operation == libc::BPF_JGE => accumulator >= instruction.k,
                        operation if operation == libc::BPF_JSET => {
                            accumulator & instruction.k != 0
                        }
                        operation => return Err(format!("jump {operation:#x}")),
                    };
                    let skip = if taken {
                        instruction.jt
                    } else {
                        instruction.jf
                    };
                    at += 1 + usize::from(skip);
                }
                code if code == libc::BPF_RET | libc::BPF_K => return Ok(instruction.k),
                code => return Err(format!("instruction {code:#x}")),
            }
        }
        Err("did not end".to_string())
    }

    /// The table from the kernel's `arch/x86/entry/syscalls/syscall_64.tbl` and
    /// `include/uapi/asm-generic/unistd.h`: (x86_64, aarch64). `None`: no such call there.
    const REFUSED: &[(&str, Option<u32>, Option<u32>)] = &[
        ("socket", Some(41), Some(198)),
        ("socketpair", Some(53), Some(199)),
        ("bind", Some(49), Some(200)),
        ("listen", Some(50), Some(201)),
        ("accept", Some(43), Some(202)),
        ("connect", Some(42), Some(203)),
        ("sendto", Some(44), Some(206)),
        ("recvfrom", Some(45), Some(207)),
        ("shutdown", Some(48), Some(210)),
        ("sendmsg", Some(46), Some(211)),
        ("recvmsg", Some(47), Some(212)),
        ("accept4", Some(288), Some(242)),
        ("recvmmsg", Some(299), Some(243)),
        ("sendmmsg", Some(307), Some(269)),
        ("io_uring_setup", Some(425), Some(425)),
        ("io_uring_enter", Some(426), Some(426)),
        ("io_uring_register", Some(427), Some(427)),
        ("fork", Some(57), None),
        ("vfork", Some(58), None),
        ("execveat", Some(322), Some(281)),
        ("memfd_create", Some(319), Some(279)),
        ("memfd_secret", Some(447), Some(447)),
        ("ptrace", Some(101), Some(117)),
        ("process_vm_readv", Some(310), Some(270)),
        ("process_vm_writev", Some(311), Some(271)),
        ("pidfd_open", Some(434), Some(434)),
        ("pidfd_getfd", Some(438), Some(438)),
        ("pidfd_send_signal", Some(424), Some(424)),
        ("process_madvise", Some(440), Some(440)),
        ("process_mrelease", Some(448), Some(448)),
        ("kcmp", Some(312), Some(272)),
        ("tkill", Some(200), Some(130)),
        ("unshare", Some(272), Some(97)),
        ("setns", Some(308), Some(268)),
        ("mount", Some(165), Some(40)),
        ("umount2", Some(166), Some(39)),
        ("pivot_root", Some(155), Some(41)),
        ("chroot", Some(161), Some(51)),
        ("open_tree", Some(428), Some(428)),
        ("move_mount", Some(429), Some(429)),
        ("fsopen", Some(430), Some(430)),
        ("fsconfig", Some(431), Some(431)),
        ("fsmount", Some(432), Some(432)),
        ("fspick", Some(433), Some(433)),
        ("mount_setattr", Some(442), Some(442)),
        ("open_tree_attr", Some(467), Some(467)),
        ("bpf", Some(321), Some(280)),
        ("userfaultfd", Some(323), Some(282)),
        ("perf_event_open", Some(298), Some(241)),
        ("keyctl", Some(250), Some(219)),
        ("add_key", Some(248), Some(217)),
        ("request_key", Some(249), Some(218)),
        ("open_by_handle_at", Some(304), Some(265)),
        ("name_to_handle_at", Some(303), Some(264)),
        ("fanotify_init", Some(300), Some(262)),
        ("fanotify_mark", Some(301), Some(263)),
        ("capget", Some(125), Some(90)),
        ("syslog", Some(103), Some(116)),
    ];
    /// The calls whose first argument is a pid (`include/uapi/asm-generic/unistd.h` and
    /// `syscall_64.tbl` again): only 0 or the tool's own pid passes.
    const PID_FIRST: &[(&str, u32, u32)] = &[
        ("prlimit64", 302, 261),
        ("sched_setaffinity", 203, 122),
        ("sched_getaffinity", 204, 123),
        ("sched_setscheduler", 144, 119),
        ("sched_getscheduler", 145, 120),
        ("sched_setparam", 142, 118),
        ("sched_getparam", 143, 121),
        ("sched_setattr", 314, 274),
        ("sched_getattr", 315, 275),
        ("sched_rr_get_interval", 148, 127),
        ("getpgid", 121, 155),
        ("getsid", 124, 156),
        ("get_robust_list", 274, 100),
        ("migrate_pages", 256, 238),
        ("move_pages", 279, 239),
    ];
    /// (name, x86_64, aarch64, the one `which` allowed): `PRIO_PROCESS` is 0 and
    /// `IOPRIO_WHO_PROCESS` is 1; the group and the user follow each.
    const WHICH_FIRST: &[(&str, u32, u32, u64)] = &[
        ("setpriority", 141, 140, 0),
        ("getpriority", 140, 141, 0),
        ("ioprio_set", 251, 30, 1),
        ("ioprio_get", 252, 31, 1),
    ];
    const FCNTL: (u32, u32) = (72, 25);
    const SETPGID: (u32, u32) = (109, 154);
    /// Calls no row names, below the ceiling: still allowed. The filter refuses what it lists;
    /// it is not a list of what a runtime may call (S2 of the fix round, recorded, not changed).
    const UNLISTED: &[(&str, u32, u32)] = &[
        ("getpid", 39, 172),
        ("statmount", 457, 457),
        ("listmount", 458, 458),
    ];
    /// The highest number the table names, `open_tree_attr`, on both architectures.
    const CEILING: u32 = 467;
    /// The calls judged by their first argument: clone, then the four signal calls.
    const CLONE: (u32, u32) = (56, 220);
    const CLONE3: (u32, u32) = (435, 435);
    const SIGNALS: &[(&str, u32, u32)] = &[
        ("kill", 62, 129),
        ("tgkill", 234, 131),
        ("rt_sigqueueinfo", 129, 138),
        ("rt_tgsigqueueinfo", 297, 240),
    ];
    /// What a runtime needs, which must stay allowed: read, write, openat, mmap, futex,
    /// exit_group, execve, and, on x86_64, the open and stat of its old ABI.
    const ALLOWED: &[(&str, Option<u32>, Option<u32>)] = &[
        ("read", Some(0), Some(63)),
        ("write", Some(1), Some(64)),
        ("openat", Some(257), Some(56)),
        ("mmap", Some(9), Some(222)),
        ("futex", Some(202), Some(98)),
        ("exit_group", Some(231), Some(94)),
        ("execve", Some(59), Some(221)),
        ("open", Some(2), None),
        ("stat", Some(4), None),
    ];
    const PID: u32 = 4242;
    /// glibc's `pthread_create`, `fork`, and `vfork` flags, and each with a namespace flag.
    const THREAD: u64 = 0x003d_0f00;
    const FORK: u64 = 0x0120_0011;
    const VFORK: u64 = 0x4111;
    const NEW_USER: u64 = 0x1000_0000;
    const NEW_NET: u64 = 0x4000_0000;

    fn patched(arch: &Arch) -> Vec<libc::sock_filter> {
        let mut program = build(arch);
        for slot in &program.pid_slots {
            program.instructions[*slot].k = PID;
        }
        program.instructions
    }

    fn pick(arch: &Arch, numbers: (Option<u32>, Option<u32>)) -> Option<u32> {
        if arch.audit_arch == AUDIT_ARCH_X86_64 {
            numbers.0
        } else {
            numbers.1
        }
    }

    /// Everything wrong with `program` for `arch`, judged against the tables of this module.
    fn faults(program: &[libc::sock_filter], arch: &Arch) -> Vec<String> {
        let found = std::cell::RefCell::new(Vec::new());
        let judge = |label: String, number: u32, audit: u32, arguments: &[u64], want: u32| match run(
            program, number, audit, arguments,
        ) {
            Ok(got) if got == want => {}
            got => found
                .borrow_mut()
                .push(format!("{}: {label}: {got:?}, not {want:#x}", arch.name)),
        };
        let expect = |label: String, number: u32, audit: u32, first: u64, want: u32| {
            judge(label, number, audit, &[first], want);
        };
        for (name, x86_64, aarch64) in REFUSED {
            if let Some(number) = pick(arch, (*x86_64, *aarch64)) {
                expect(name.to_string(), number, arch.audit_arch, 0, EPERM);
            }
        }
        for (name, x86_64, aarch64) in ALLOWED {
            if let Some(number) = pick(arch, (*x86_64, *aarch64)) {
                expect(name.to_string(), number, arch.audit_arch, 0, ALLOW);
            }
        }
        for (name, x86_64, aarch64) in UNLISTED {
            let number = pick(arch, (Some(*x86_64), Some(*aarch64))).unwrap();
            expect(
                format!("unlisted {name}"),
                number,
                arch.audit_arch,
                0,
                ALLOW,
            );
        }
        let mut above = vec![CEILING + 1, CEILING + 2, 999, X32_SYSCALL_BIT - 1];
        if arch.audit_arch == AUDIT_ARCH_AARCH64 {
            above.push(u32::MAX);
        }
        for number in above {
            expect(
                format!("call {number}, above the ceiling"),
                number,
                arch.audit_arch,
                0,
                ENOSYS,
            );
        }
        let pid = u64::from(PID);
        let other_pids = [1, pid + 1, u64::MAX, (-i64::from(PID)) as u64];
        for (name, x86_64, aarch64) in PID_FIRST {
            let number = pick(arch, (Some(*x86_64), Some(*aarch64))).unwrap();
            for own in [0, pid] {
                expect(
                    format!("{name} of {own}"),
                    number,
                    arch.audit_arch,
                    own,
                    ALLOW,
                );
            }
            for other in other_pids {
                expect(
                    format!("{name} of {other}"),
                    number,
                    arch.audit_arch,
                    other,
                    EPERM,
                );
            }
        }
        for (name, x86_64, aarch64, which) in WHICH_FIRST {
            let number = pick(arch, (Some(*x86_64), Some(*aarch64))).unwrap();
            for own in [0, pid] {
                judge(
                    format!("{name} of process {own}"),
                    number,
                    arch.audit_arch,
                    &[*which, own],
                    ALLOW,
                );
            }
            for other in other_pids {
                judge(
                    format!("{name} of process {other}"),
                    number,
                    arch.audit_arch,
                    &[*which, other],
                    EPERM,
                );
            }
            for (label, group_or_user) in [("group", *which + 1), ("user", *which + 2)] {
                for who in [0, pid] {
                    judge(
                        format!("{name} of {label} {who}"),
                        number,
                        arch.audit_arch,
                        &[group_or_user, who],
                        EPERM,
                    );
                }
            }
        }
        let fcntl = pick(arch, (Some(FCNTL.0), Some(FCNTL.1))).unwrap();
        let (f_getfl, f_setfl, f_getown, f_setown, f_setsig, f_setown_ex, f_getown_ex) =
            (3, 4, 9, 8, 10, 15, 16);
        for (label, command, argument, want) in [
            ("F_GETFL", f_getfl, 0, ALLOW),
            ("F_SETFL O_ASYNC", f_setfl, 0o20000, ALLOW),
            ("F_GETOWN", f_getown, 0, ALLOW),
            ("F_SETSIG SIGKILL", f_setsig, 9, ALLOW),
            ("F_GETOWN_EX", f_getown_ex, 0x7fff_0000, ALLOW),
            ("F_SETOWN to nobody", f_setown, 0, ALLOW),
            ("F_SETOWN to itself", f_setown, pid, ALLOW),
            ("F_SETOWN to pid 1", f_setown, 1, EPERM),
            ("F_SETOWN to another", f_setown, pid + 1, EPERM),
            (
                "F_SETOWN to its group",
                f_setown,
                (-i64::from(PID)) as u64,
                EPERM,
            ),
            ("F_SETOWN_EX", f_setown_ex, 0x7fff_0000, EPERM),
        ] {
            judge(
                format!("fcntl {label}"),
                fcntl,
                arch.audit_arch,
                &[3, command, argument],
                want,
            );
        }
        let setpgid = pick(arch, (Some(SETPGID.0), Some(SETPGID.1))).unwrap();
        for (pid_argument, group, want) in [
            (0, 0, ALLOW),
            (0, 1, EPERM),
            (0, pid + 1, EPERM),
            (pid, pid, EPERM),
            (1, 0, EPERM),
        ] {
            judge(
                format!("setpgid({pid_argument}, {group})"),
                setpgid,
                arch.audit_arch,
                &[pid_argument, group],
                want,
            );
        }
        let clone = pick(arch, (Some(CLONE.0), Some(CLONE.1))).unwrap();
        for (label, flags, want) in [
            ("a thread", THREAD, ALLOW),
            ("a fork", FORK, EPERM),
            ("a vfork", VFORK, EPERM),
            ("a bare SIGCHLD", 0x11, EPERM),
            ("a thread in a new user namespace", THREAD | NEW_USER, EPERM),
            ("a thread in a new network", THREAD | NEW_NET, EPERM),
            ("a thread flag in the high half only", THREAD << 32, EPERM),
        ] {
            expect(
                format!("clone, {label}"),
                clone,
                arch.audit_arch,
                flags,
                want,
            );
        }
        let clone3 = pick(arch, (Some(CLONE3.0), Some(CLONE3.1))).unwrap();
        expect("clone3".to_string(), clone3, arch.audit_arch, 0, ENOSYS);
        for (name, x86_64, aarch64) in SIGNALS {
            let number = pick(arch, (Some(*x86_64), Some(*aarch64))).unwrap();
            for (label, target, want) in [
                ("itself", u64::from(PID), ALLOW),
                ("pid 1", 1, EPERM),
                ("every process", u64::MAX, EPERM),
                ("its group", 0, EPERM),
                ("another group", (-i64::from(PID)) as u64, EPERM),
            ] {
                expect(
                    format!("{name} to {label}"),
                    number,
                    arch.audit_arch,
                    target,
                    want,
                );
            }
        }
        let foreign = if arch.audit_arch == AUDIT_ARCH_X86_64 {
            [AUDIT_ARCH_I386, AUDIT_ARCH_AARCH64]
        } else {
            [AUDIT_ARCH_ARM, AUDIT_ARCH_X86_64]
        };
        for audit in foreign {
            for number in [0, 41, 59, 198, 221] {
                expect(
                    format!("ABI {audit:#x} call {number}"),
                    number,
                    audit,
                    0,
                    KILL_PROCESS,
                );
            }
        }
        if arch.audit_arch == AUDIT_ARCH_X86_64 {
            for number in [0, 1, 41, 59, 512, 545] {
                let x32 = X32_SYSCALL_BIT | number;
                expect(
                    format!("x32 call {number}"),
                    x32,
                    arch.audit_arch,
                    0,
                    KILL_PROCESS,
                );
            }
        }
        found.into_inner()
    }

    #[test]
    fn the_ceiling_is_the_highest_number_named() {
        for arch in [&X86_64, &AARCH64] {
            assert_eq!(super::ceiling(arch), CEILING, "{}", arch.name);
        }
    }

    #[test]
    fn the_program_of_each_architecture_does_what_the_kernels_tables_say() {
        for arch in [&X86_64, &AARCH64] {
            let faults = faults(&patched(arch), arch);
            assert!(faults.is_empty(), "{faults:#?}");
        }
    }

    /// The table typed here names every row of the table above, by name and by number, so a row
    /// added there without a test here does not pass.
    #[test]
    fn the_two_tables_name_the_same_calls() {
        for arch in [&X86_64, &AARCH64] {
            for entry in arch.entries {
                let typed = REFUSED
                    .iter()
                    .find(|(name, _, _)| *name == entry.name)
                    .map(|(_, x86_64, aarch64)| pick(arch, (*x86_64, *aarch64)))
                    .or_else(|| {
                        SIGNALS
                            .iter()
                            .find(|(name, _, _)| *name == entry.name)
                            .map(|(_, x86_64, aarch64)| pick(arch, (Some(*x86_64), Some(*aarch64))))
                    })
                    .or_else(|| {
                        (entry.name == "clone").then(|| pick(arch, (Some(CLONE.0), Some(CLONE.1))))
                    })
                    .or_else(|| {
                        (entry.name == "clone3")
                            .then(|| pick(arch, (Some(CLONE3.0), Some(CLONE3.1))))
                    })
                    .or_else(|| {
                        PID_FIRST
                            .iter()
                            .find(|(name, _, _)| *name == entry.name)
                            .map(|(_, x86_64, aarch64)| pick(arch, (Some(*x86_64), Some(*aarch64))))
                    })
                    .or_else(|| {
                        WHICH_FIRST
                            .iter()
                            .find(|(name, _, _, _)| *name == entry.name)
                            .map(|(_, x86_64, aarch64, _)| {
                                pick(arch, (Some(*x86_64), Some(*aarch64)))
                            })
                    })
                    .or_else(|| {
                        (entry.name == "fcntl").then(|| pick(arch, (Some(FCNTL.0), Some(FCNTL.1))))
                    })
                    .or_else(|| {
                        (entry.name == "setpgid")
                            .then(|| pick(arch, (Some(SETPGID.0), Some(SETPGID.1))))
                    });
                assert_eq!(typed, Some(entry.number), "{} {}", arch.name, entry.name);
            }
            let mut numbers = std::collections::BTreeSet::new();
            for number in arch.entries.iter().filter_map(|entry| entry.number) {
                assert!(numbers.insert(number), "{} repeats {number}", arch.name);
            }
        }
    }

    /// The check above fails a program that is wrong in each of these ways.
    #[test]
    fn a_deliberately_wrong_program_is_caught() {
        type Damage = fn(&mut Vec<libc::sock_filter>);
        let damages: [(&str, Damage); 13] = [
            ("no architecture check", |program| {
                program[1].jf = 1;
            }),
            ("a refusal that allows", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.k == ERRNO | 1)
                    .unwrap();
                program[at].k = ALLOW;
            }),
            ("a block jumped past", |program| {
                let at = program
                    .iter()
                    .position(|instruction| {
                        instruction.jf == 1 && instruction.k > 0 && instruction.k < 1000
                    })
                    .unwrap();
                program[at].jt = 1;
            }),
            ("clone with any flags", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.jf == 5)
                    .unwrap();
                program[at + 1] = super::statement(super::RETURN, ALLOW);
            }),
            ("a pid never written", |_| {}),
            ("a row dropped", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.k == 41 && instruction.jf == 1)
                    .or_else(|| {
                        program
                            .iter()
                            .position(|instruction| instruction.k == 198 && instruction.jf == 1)
                    })
                    .unwrap();
                program.drain(at..at + 2);
            }),
            ("no ceiling", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.k == CEILING + 1)
                    .unwrap();
                program.drain(at..at + 2);
            }),
            ("every unlisted call refused", |program| {
                program.last_mut().unwrap().k = ERRNO | 1;
            }),
            ("a pid check that passes any pid", |program| {
                let at = program
                    .iter()
                    .position(|instruction| {
                        instruction.k == PID && instruction.jt == 1 && instruction.jf == 0
                    })
                    .unwrap();
                program[at].jf = 1;
            }),
            ("a priority of any group or user", |program| {
                for instruction in program.iter_mut().filter(|instruction| instruction.jf == 3) {
                    instruction.jf = 0;
                }
            }),
            ("an owner anyone may be", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.k == 8 && instruction.jf == 4)
                    .unwrap();
                program[at].jt = 4;
            }),
            ("F_SETOWN_EX let through", |program| {
                let at = program
                    .iter()
                    .position(|instruction| instruction.k == 15 && instruction.jt == 4)
                    .unwrap();
                program[at].jt = 0;
            }),
            ("a group anyone's", |program| {
                let at = program
                    .iter()
                    .position(|instruction| {
                        instruction.k == 0 && instruction.jt == 1 && instruction.jf == 0
                    })
                    .unwrap();
                program[at].jf = 1;
            }),
        ];
        for arch in [&X86_64, &AARCH64] {
            for (label, damage) in damages {
                let mut program = if label == "a pid never written" {
                    build(arch).instructions
                } else {
                    patched(arch)
                };
                damage(&mut program);
                assert!(!faults(&program, arch).is_empty(), "{} {label}", arch.name);
            }
        }
    }

    #[test]
    fn only_x86_64_and_aarch64_have_a_table() {
        let native = super::native().map(|arch| arch.name);
        if cfg!(target_arch = "x86_64") {
            assert_eq!(native, Some("x86_64"));
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(native, Some("aarch64"));
        } else {
            assert_eq!(native, None);
        }
    }
}
