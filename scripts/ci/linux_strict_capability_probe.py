#!/usr/bin/env python3
"""Record the kernel capabilities the Linux strict isolation backend (#982) needs.

The script prints one line per item and measures each capability with the real
syscall, as a non-root user, in a fresh single-threaded child process (this
script run again with ``--child``):

- the runner image, the kernel release and the active LSM list;
- the Landlock ABI, and a Landlock domain that really refuses a read;
- ``kernel.unprivileged_userns_clone`` and
  ``kernel.apparmor_restrict_unprivileged_userns``;
- user, mount, net and IPC namespaces, a uid/gid map to a non-zero inner uid, a
  tmpfs mount and ``pivot_root`` inside them;
- a seccomp filter that refuses one syscall with its errno;
- ``close_range`` with ``CLOSE_RANGE_CLOEXEC``.

Item states: ``ok`` (present), ``missing`` (measured and refused, with the
errno), ``blocked`` (not reached because an earlier step is missing),
``recorded`` (a value only), ``n/a-in-container`` (``--context container``
only, for a file a container cannot see) and ``error`` (could not be
measured). ``error`` always fails the run. ``--phase initial`` records
``missing`` and ``blocked`` items and says so in its summary
(``result=recorded``); ``--phase prepared`` fails on them too. A timeout is an
``error``, never a pass, and nothing is skipped.

``--context github-runner`` is what CI runs. ``--context container`` exists
only to check this script inside a local container, where securityfs is not
mounted and the GitHub image variables are absent; it tolerates exactly those
two absences and nothing else.
"""

import argparse
import ctypes
import errno
import fcntl
import json
import os
import subprocess
import sys
import tempfile

# Syscall numbers and the seccomp audit architecture, per architecture. The
# asm-generic numbers (Landlock, close_range) happen to agree on both; they are
# still listed per architecture so that no number is assumed.
ARCHES = {
    "x86_64": {
        "audit_arch": 0xC000003E,  # AUDIT_ARCH_X86_64
        "x32_syscall_bit": 0x40000000,  # __X32_SYSCALL_BIT
        "getppid": 110,
        "getpid": 39,
        "pivot_root": 155,
        "close_range": 436,
        "landlock_create_ruleset": 444,
        "landlock_restrict_self": 446,
    },
    "aarch64": {
        "audit_arch": 0xC00000B7,  # AUDIT_ARCH_AARCH64
        "x32_syscall_bit": None,
        "getppid": 173,
        "getpid": 172,
        "pivot_root": 41,
        "close_range": 436,
        "landlock_create_ruleset": 444,
        "landlock_restrict_self": 446,
    },
}

CLONE_NEWNS = 0x00020000
CLONE_NEWIPC = 0x08000000
CLONE_NEWUSER = 0x10000000
CLONE_NEWNET = 0x40000000
MS_RDONLY = 0x1
MS_REMOUNT = 0x20
MS_REC = 0x4000
MS_PRIVATE = 0x40000
MNT_DETACH = 0x2
PR_SET_NO_NEW_PRIVS = 38
PR_SET_SECCOMP = 22
SECCOMP_MODE_FILTER = 2
SECCOMP_RET_KILL_PROCESS = 0x80000000
SECCOMP_RET_ERRNO = 0x00050000
SECCOMP_RET_ALLOW = 0x7FFF0000
BPF_LD_W_ABS = 0x20
BPF_JEQ_K = 0x15
BPF_JGE_K = 0x35
BPF_RET_K = 0x06
SECCOMP_DATA_NR = 0
SECCOMP_DATA_ARCH = 4
LANDLOCK_CREATE_RULESET_VERSION = 1
LANDLOCK_ACCESS_FS_READ_FILE = 1 << 2
CLOSE_RANGE_CLOEXEC = 1 << 2
INNER_ID = 1000
CHILD_TIMEOUT_SECONDS = 20.0

LIBC = ctypes.CDLL(None, use_errno=True)
LIBC.syscall.restype = ctypes.c_long


class SockFilter(ctypes.Structure):
    _fields_ = [
        ("code", ctypes.c_ushort),
        ("jt", ctypes.c_ubyte),
        ("jf", ctypes.c_ubyte),
        ("k", ctypes.c_uint32),
    ]


class SockFprog(ctypes.Structure):
    _fields_ = [("len", ctypes.c_ushort), ("filter", ctypes.POINTER(SockFilter))]


class LandlockRulesetAttr(ctypes.Structure):
    _fields_ = [("handled_access_fs", ctypes.c_uint64)]


def errno_name(number):
    return "%s(%d)" % (errno.errorcode.get(number, "E?"), number)


def call(result):
    """The return value of a libc call and, when it failed, its errno."""
    return result, (ctypes.get_errno() if result < 0 else 0)


def syscall(number, *args):
    ctypes.set_errno(0)
    return call(LIBC.syscall(ctypes.c_long(number), *args))


def libc_call(function, *args):
    ctypes.set_errno(0)
    return call(function(*args))


def outcome(status, detail):
    return {"status": status, "detail": detail}


def in_child(name, *args):
    """Run the measurement ``name`` in a fresh, single-threaded child process
    (this script again, with ``--child``) and return the dict it printed.

    A child that ends without a result (a signal, a seccomp kill), exits
    non-zero, or runs past the deadline is an ``error``: nothing about the
    capability was shown. On the deadline ``subprocess.run`` kills and reaps
    the child it started.
    """
    command = [sys.executable, "-I", "-B", os.path.abspath(__file__), "--child", name, *args]
    try:
        done = subprocess.run(command, capture_output=True, timeout=CHILD_TIMEOUT_SECONDS, check=False)
    except subprocess.TimeoutExpired:
        return {"error": "the child ran past %ss (a timeout is not evidence)" % CHILD_TIMEOUT_SECONDS}
    stderr = done.stderr.decode(errors="replace").strip()[-300:]
    if done.returncode < 0:
        return {"error": "the child ended by signal %d %s" % (-done.returncode, stderr)}
    if done.returncode != 0 or not done.stdout.strip():
        return {"error": "the child gave no result (exit %d) %s" % (done.returncode, stderr)}
    try:
        return json.loads(done.stdout)
    except ValueError:
        return {"error": "the child printed no JSON result %s" % stderr}


def child_main(name, args):
    """The ``--child`` entry point: one measurement, its dict printed as JSON."""
    arch = ARCHES.get(os.uname().machine)
    factories = {
        "landlock_enforce": lambda: landlock_enforce_child(arch, args[0]),
        "namespaces": lambda: namespace_child(arch, args[0]),
        "seccomp": lambda: seccomp_child(arch),
        "close_range": lambda: close_range_child(arch),
    }
    try:
        result = factories[name]()()
    except Exception as exc:  # reported as an error item, never swallowed
        result = {"error": "%s: %s" % (type(exc).__name__, exc)}
    sys.stdout.write(json.dumps(result))
    sys.stdout.flush()
    return 0


def child_result(result, *names):
    """Map a child's dict to one outcome per item name."""
    if "error" in result:
        return [outcome("error", result["error"]) for _ in names]
    return [outcome(result[name]["status"], result[name]["detail"]) for name in names]


def read_text(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read().strip()


def item_user(_context, _arch):
    uid, euid, gid = os.getuid(), os.geteuid(), os.getgid()
    detail = "uid=%d euid=%d gid=%d" % (uid, euid, gid)
    if euid == 0 or uid == 0:
        return outcome("error", detail + ": must run as a non-root user to be evidence")
    return outcome("recorded", detail)


def item_image(context, _arch):
    try:
        os_release = dict(
            line.split("=", 1) for line in read_text("/etc/os-release").splitlines() if "=" in line
        )
    except OSError as exc:
        return outcome("error", "/etc/os-release unreadable: " + errno_name(exc.errno))
    pretty = os_release.get("PRETTY_NAME", "").strip('"')
    image_os = os.environ.get("ImageOS")
    image_version = os.environ.get("ImageVersion")
    detail = "os=%r ImageOS=%s ImageVersion=%s" % (pretty, image_os, image_version)
    if not pretty:
        return outcome("error", detail + ": no PRETTY_NAME")
    if context == "github-runner" and not (image_os and image_version):
        return outcome("error", detail + ": the GitHub image variables are not set")
    return outcome("recorded", detail)


def item_kernel(_context, arch):
    uname = os.uname()
    detail = "release=%s machine=%s" % (uname.release, uname.machine)
    if arch is None:
        return outcome("error", detail + ": no syscall table for this architecture")
    return outcome("recorded", detail)


def item_lsm(context, _arch):
    path = "/sys/kernel/security/lsm"
    try:
        value = read_text(path)
    except OSError as exc:
        if context == "container" and exc.errno == errno.ENOENT:
            return outcome("n/a-in-container", "%s: %s (securityfs is not mounted here)" % (path, errno_name(exc.errno)))
        return outcome("error", "%s: %s" % (path, errno_name(exc.errno)))
    names = value.split(",")
    return outcome("ok" if "landlock" in names else "missing", "lsm=" + value)


def item_sysctl(name):
    def measure(_context, _arch):
        path = "/proc/sys/kernel/" + name
        try:
            return outcome("recorded", "kernel.%s=%s" % (name, read_text(path)))
        except OSError as exc:
            if exc.errno == errno.ENOENT:
                return outcome("recorded", "kernel.%s absent (no such sysctl on this kernel)" % name)
            return outcome("error", "kernel.%s unreadable: %s" % (name, errno_name(exc.errno)))

    return measure


def item_landlock_abi(_context, arch):
    abi, err = syscall(
        arch["landlock_create_ruleset"],
        ctypes.c_void_p(0),
        ctypes.c_size_t(0),
        ctypes.c_uint32(LANDLOCK_CREATE_RULESET_VERSION),
    )
    if abi < 1:
        return outcome("missing", "landlock_create_ruleset(VERSION) -> %s" % errno_name(err))
    return outcome("ok", "abi=%d" % abi)


def landlock_enforce_child(arch, path):
    def run():
        with open(path, "rb") as handle:
            handle.read(1)
        attr = LandlockRulesetAttr(LANDLOCK_ACCESS_FS_READ_FILE)
        ruleset, err = syscall(
            arch["landlock_create_ruleset"],
            ctypes.byref(attr),
            ctypes.c_size_t(ctypes.sizeof(attr)),
            ctypes.c_uint32(0),
        )
        if ruleset < 0:
            return {"landlock_enforce": outcome("missing", "create_ruleset -> " + errno_name(err))}
        rc, err = libc_call(LIBC.prctl, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        if rc != 0:
            return {"landlock_enforce": outcome("missing", "PR_SET_NO_NEW_PRIVS -> " + errno_name(err))}
        rc, err = syscall(arch["landlock_restrict_self"], ctypes.c_int(ruleset), ctypes.c_uint32(0))
        if rc != 0:
            return {"landlock_enforce": outcome("missing", "restrict_self -> " + errno_name(err))}
        try:
            with open(path, "rb") as handle:
                handle.read(1)
        except OSError as exc:
            status = "ok" if exc.errno == errno.EACCES else "missing"
            return {"landlock_enforce": outcome(status, "read before: ok; read after restrict_self: " + errno_name(exc.errno))}
        return {"landlock_enforce": outcome("missing", "read after restrict_self still succeeded")}

    return run


def item_landlock_enforce(_context, arch):
    return child_result(in_child("landlock_enforce", "/etc/os-release"), "landlock_enforce")[0]


NAMESPACE_ITEMS = ("namespaces", "id_map", "tmpfs_mount", "pivot_root")


def namespace_child(arch, new_root):
    def run():
        result = {}

        def stop(at, detail):
            result[at] = outcome("missing", detail)
            for name in NAMESPACE_ITEMS[NAMESPACE_ITEMS.index(at) + 1 :]:
                result[name] = outcome("blocked", "not reached: %s is missing" % at)
            return result

        uid, gid = os.getuid(), os.getgid()
        flags = CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWNET | CLONE_NEWIPC
        rc, err = libc_call(LIBC.unshare, flags)
        if rc != 0:
            return stop("namespaces", "unshare(USER|NS|NET|IPC) -> " + errno_name(err))
        result["namespaces"] = outcome("ok", "unshare(USER|NS|NET|IPC)=0")
        try:
            for name, text in (
                ("setgroups", "deny"),
                ("uid_map", "%d %d 1" % (INNER_ID, uid)),
                ("gid_map", "%d %d 1" % (INNER_ID, gid)),
            ):
                with open("/proc/self/" + name, "w", encoding="ascii") as handle:
                    handle.write(text)
        except OSError as exc:
            return stop("id_map", "write /proc/self/%s -> %s" % (name, errno_name(exc.errno)))
        if (os.getuid(), os.getgid()) != (INNER_ID, INNER_ID):
            return stop("id_map", "inner ids are %d/%d, not %d" % (os.getuid(), os.getgid(), INNER_ID))
        result["id_map"] = outcome("ok", "setgroups=deny, host %d:%d -> inner %d:%d" % (uid, gid, INNER_ID, INNER_ID))
        rc, err = libc_call(LIBC.mount, None, b"/", None, MS_REC | MS_PRIVATE, None)
        if rc != 0:
            return stop("tmpfs_mount", "mount(/, MS_REC|MS_PRIVATE) -> " + errno_name(err))
        root = new_root.encode()
        rc, err = libc_call(LIBC.mount, b"tmpfs", root, b"tmpfs", 0, b"size=1m,mode=0755")
        if rc != 0:
            return stop("tmpfs_mount", "mount(tmpfs) -> " + errno_name(err))
        result["tmpfs_mount"] = outcome("ok", "mount(tmpfs)=0 at a private mount point")
        os.mkdir(os.path.join(new_root, "old"))
        rc, err = syscall(arch["pivot_root"], ctypes.c_char_p(root), ctypes.c_char_p(root + b"/old"))
        if rc != 0:
            return stop("pivot_root", "pivot_root -> " + errno_name(err))
        os.chdir("/")
        rc, err = libc_call(LIBC.umount2, b"/old", MNT_DETACH)
        if rc != 0:
            return stop("pivot_root", "umount2(old root, MNT_DETACH) -> " + errno_name(err))
        os.rmdir("/old")
        rc, err = libc_call(LIBC.mount, None, b"/", None, MS_REMOUNT | MS_RDONLY, None)
        if rc != 0:
            return stop("pivot_root", "remount new root read-only -> " + errno_name(err))
        entries = os.listdir("/")
        if entries:
            return stop("pivot_root", "the new root is not empty: %r" % entries)
        try:
            os.stat("/etc/os-release")
        except OSError as exc:
            hidden = errno_name(exc.errno)
        else:
            return stop("pivot_root", "the old root is still reachable")
        result["pivot_root"] = outcome(
            "ok", "pivot_root=0, old root detached, read-only remount=0, host /etc/os-release -> " + hidden
        )
        return result

    return run


def measure_namespaces(arch):
    new_root = tempfile.mkdtemp(prefix="ca-strict-probe-")
    try:
        return child_result(in_child("namespaces", new_root), *NAMESPACE_ITEMS)
    finally:
        os.rmdir(new_root)


def seccomp_child(arch):
    def run():
        before, err = syscall(arch["getppid"])
        if before <= 0:
            return {"seccomp": outcome("error", "getppid before the filter -> " + errno_name(err))}
        program = [(BPF_LD_W_ABS, 0, 0, SECCOMP_DATA_ARCH), (BPF_JEQ_K, 1, 0, arch["audit_arch"])]
        program.append((BPF_RET_K, 0, 0, SECCOMP_RET_KILL_PROCESS))
        program.append((BPF_LD_W_ABS, 0, 0, SECCOMP_DATA_NR))
        if arch["x32_syscall_bit"] is not None:
            program.append((BPF_JGE_K, 0, 1, arch["x32_syscall_bit"]))
            program.append((BPF_RET_K, 0, 0, SECCOMP_RET_KILL_PROCESS))
        program.append((BPF_JEQ_K, 0, 1, arch["getppid"]))
        program.append((BPF_RET_K, 0, 0, SECCOMP_RET_ERRNO | errno.EPERM))
        program.append((BPF_RET_K, 0, 0, SECCOMP_RET_ALLOW))
        filters = (SockFilter * len(program))(*[SockFilter(*entry) for entry in program])
        fprog = SockFprog(len(program), filters)
        rc, err = libc_call(LIBC.prctl, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        if rc != 0:
            return {"seccomp": outcome("missing", "PR_SET_NO_NEW_PRIVS -> " + errno_name(err))}
        rc, err = libc_call(LIBC.prctl, PR_SET_SECCOMP, SECCOMP_MODE_FILTER, ctypes.byref(fprog), 0, 0)
        if rc != 0:
            return {"seccomp": outcome("missing", "PR_SET_SECCOMP(FILTER) -> " + errno_name(err))}
        after, err = syscall(arch["getppid"])
        allowed, _ = syscall(arch["getpid"])
        if after == -1 and err == errno.EPERM and allowed > 0:
            return {"seccomp": outcome("ok", "filter installed; getppid -> %s (before: %d); getpid still allowed" % (errno_name(err), before))}
        return {"seccomp": outcome("missing", "getppid after the filter -> ret=%d %s" % (after, errno_name(err)))}

    return run


def item_seccomp(_context, arch):
    return child_result(in_child("seccomp"), "seccomp")[0]


def close_range_child(arch):
    def run():
        fd = os.open("/dev/null", os.O_RDONLY)
        os.set_inheritable(fd, True)
        if fcntl.fcntl(fd, fcntl.F_GETFD) & fcntl.FD_CLOEXEC:
            return {"close_range": outcome("error", "could not clear FD_CLOEXEC for the control")}
        rc, err = syscall(
            arch["close_range"],
            ctypes.c_uint(fd),
            ctypes.c_uint(0xFFFFFFFF),
            ctypes.c_uint(CLOSE_RANGE_CLOEXEC),
        )
        if rc != 0:
            return {"close_range": outcome("missing", "close_range(CLOEXEC) -> " + errno_name(err))}
        if fcntl.fcntl(fd, fcntl.F_GETFD) & fcntl.FD_CLOEXEC:
            return {"close_range": outcome("ok", "close_range(fd, ~0, CLOEXEC)=0; FD_CLOEXEC clear before, set after")}
        return {"close_range": outcome("missing", "close_range returned 0 but FD_CLOEXEC is not set")}

    return run


def item_close_range(_context, arch):
    return child_result(in_child("close_range"), "close_range")[0]


def main(argv):
    if argv[:1] == ["--child"]:
        return child_main(argv[1], argv[2:])
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--phase", required=True, choices=("initial", "prepared"))
    parser.add_argument("--context", required=True, choices=("github-runner", "container"))
    args = parser.parse_args(argv)

    arch = ARCHES.get(os.uname().machine)
    print("probe: phase=%s context=%s python=%s" % (args.phase, args.context, sys.version.split()[0]))
    rows = []
    for name, measure in (
        ("user", item_user),
        ("image", item_image),
        ("kernel", item_kernel),
        ("lsm", item_lsm),
        ("sysctl_unprivileged_userns_clone", item_sysctl("unprivileged_userns_clone")),
        ("sysctl_apparmor_restrict_unprivileged_userns", item_sysctl("apparmor_restrict_unprivileged_userns")),
    ):
        rows.append((name, measure(args.context, arch)))
    if arch is None:
        for name in ("landlock_abi", "landlock_enforce") + NAMESPACE_ITEMS + ("seccomp", "close_range"):
            rows.append((name, outcome("error", "no syscall table for %s" % os.uname().machine)))
    else:
        rows.append(("landlock_abi", item_landlock_abi(args.context, arch)))
        rows.append(("landlock_enforce", item_landlock_enforce(args.context, arch)))
        rows.extend(zip(NAMESPACE_ITEMS, measure_namespaces(arch)))
        rows.append(("seccomp", item_seccomp(args.context, arch)))
        rows.append(("close_range", item_close_range(args.context, arch)))

    errors, missing = [], []
    for name, result in rows:
        print("%s %s: %s: %s" % (args.phase, name, result["status"], result["detail"]))
        if result["status"] == "error":
            errors.append(name)
        elif result["status"] in ("missing", "blocked"):
            missing.append(name)
    if args.phase == "initial":
        # The initial phase records what the runner offers before the preparation:
        # a missing capability is recorded, never a pass and never a failure.
        # Only an item that could not be measured fails it.
        print(
            "summary: phase=initial result=%s missing=%s errors=%s (record only; missing is not a failure here)"
            % ("fail" if errors else "recorded", ",".join(missing) or "none", ",".join(errors) or "none")
        )
        return 1 if errors else 0
    failures = errors + missing
    print(
        "summary: phase=prepared result=%s failures=%s"
        % ("fail" if failures else "pass", ",".join(failures) or "none")
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
