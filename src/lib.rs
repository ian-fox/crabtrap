pub use config::{Check, Config, ConfigEntry};
pub use map::MemoryMap;
use nix::{
    errno::Errno,
    libc::c_int,
    sys::{
        ptrace::{
            getevent, getregs, kill, read, setoptions, syscall, traceme, AddressType, Event,
            Options,
        },
        signal::Signal,
        wait::{waitpid, WaitStatus},
    },
    unistd::{execve, fork, ForkResult, Pid},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CStr,
};
use syscalls::Sysno;
mod config;
mod map;

fn event_from_int(event: i32) -> Event {
    match event {
        1 => Event::PTRACE_EVENT_FORK,
        2 => Event::PTRACE_EVENT_VFORK,
        3 => Event::PTRACE_EVENT_CLONE,
        4 => Event::PTRACE_EVENT_EXEC,
        5 => Event::PTRACE_EVENT_VFORK_DONE,
        6 => Event::PTRACE_EVENT_EXIT,
        7 => Event::PTRACE_EVENT_SECCOMP,
        128 => Event::PTRACE_EVENT_STOP,
        e => panic!("Unknown ptrace event {e}"),
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum ChildExit {
    Exited(i32),
    IllegalSyscall(Sysno, String),
}

/// child sets up ptrace and then calls execve.
fn child(path: &CStr, args: &[&CStr], env: &[&CStr]) -> ! {
    // Unsafe to use `println!` (or `unwrap`) here. See https://docs.rs/nix/latest/nix/unistd/fn.fork.html#safety
    // Since we're not handling errors anyway, panics should be fine for now.

    traceme().expect("error calling traceme");
    execve(path, args, env).expect("error calling execve");
    unreachable!();
}

/// handle_syscall walks up the stack to see where a syscall came from, and returns an IllegalSyscall if it should be blocked.
///
/// Reference: https://github.com/ARM-software/abi-aa/blob/2a70c42d62e9c3eb5887fa50b71257f20daca6f9/aapcs64/aapcs64.rst#646the-frame-pointer
fn handle_syscall(pid: Pid, config: &Config, map: &mut MemoryMap) -> Option<ChildExit> {
    let regs = getregs(pid).expect("failed to get registers");
    let syscall = Sysno::from(regs.regs[8] as u32);

    // I don't have an exhaustive knowledge of which syscalls might affect memory.
    // For a real project I'd do more research or set up some tests to see if I'd missed any.
    if BTreeSet::from([
        Sysno::execve,
        Sysno::execveat,
        Sysno::clone,
        Sysno::mmap,
        Sysno::munmap,
        Sysno::mremap,
    ])
    .contains(&syscall)
    {
        *map = MemoryMap::from_pid(pid).unwrap();
    }

    for addr in [regs.pc, regs.regs[30]] {
        if let Some(loc) = map.lookup(addr) {
            match config.check(loc, syscall) {
                Check::Allowed => return None,
                Check::Blocked => return Some(ChildExit::IllegalSyscall(syscall, loc.to_string())),
                Check::Unknown => {}
            }
        }
    }

    let mut frame_pointer: u64 = regs.regs[29];
    let mut saved_lr;
    loop {
        if frame_pointer == 0 {
            break;
        }

        saved_lr =
            read(pid, (frame_pointer + 8) as AddressType).expect("failed to read saved lr") as u64;

        if let Some(loc) = map.lookup(saved_lr) {
            match config.check(loc, syscall) {
                Check::Allowed => return None,
                Check::Blocked => return Some(ChildExit::IllegalSyscall(syscall, loc.to_string())),
                Check::Unknown => {}
            }
        }

        frame_pointer =
            read(pid, frame_pointer as AddressType).expect("failed to read frame pointer") as u64;
    }

    None
}

/// parent attaches to the child with ptrace and then watches for syscalls in a loop
fn parent(child: Pid, config: &Config) -> ChildExit {
    println!("Continuing execution in parent process, new child has pid: {child}");

    // Wait for the stop from the first exec
    waitpid(child, None).expect("failed to waitpid");

    setoptions(
        child,
        Options::PTRACE_O_EXITKILL
            .union(Options::PTRACE_O_TRACESYSGOOD)
            .union(Options::PTRACE_O_TRACEFORK)
            .union(Options::PTRACE_O_TRACECLONE)
            .union(Options::PTRACE_O_TRACEVFORK)
            .union(Options::PTRACE_O_TRACEEXEC),
    )
    .expect("failed to set ptrace options");

    let mut children: BTreeMap<Pid, Box<MemoryMap>> =
        BTreeMap::from([(child, Box::new(MemoryMap::from_pid(child).unwrap()))]);
    let mut ignore_next_stop: BTreeSet<Pid> = BTreeSet::new();
    let mut child_exit = None;

    println!("Starting to watch child...");
    syscall(child, None).expect("failed to start child");

    loop {
        match waitpid(None, None) {
            Err(Errno::ECHILD) => {
                return ChildExit::Exited(
                    child_exit.unwrap_or_else(|| panic!("unknown exit status for child {child}")),
                )
            }
            Ok(WaitStatus::Exited(pid, code)) => {
                if pid == child {
                    child_exit = Some(code);
                }
            }
            Ok(WaitStatus::PtraceSyscall(pid)) => {
                let child_mem: &mut MemoryMap = children
                    .entry(pid)
                    .or_insert(Box::new(MemoryMap::from_pid(pid).unwrap_or_else(|e| {
                        panic!("Couldn't build map for {}: {}", pid, e)
                    })));

                if let Some(exit) = handle_syscall(pid, config, child_mem) {
                    kill(pid).unwrap_or_else(|e| panic!("failed to kill child {pid}: {e}"));
                    return exit;
                }
                syscall(pid, None)
                    .unwrap_or_else(|e| panic!("failed to restart child {pid} after syscall: {e}"));
            }
            Ok(WaitStatus::Stopped(pid, signal)) => {
                if signal == Signal::SIGSTOP && ignore_next_stop.contains(&pid) {
                    ignore_next_stop.remove(&pid);
                    syscall(pid, None).unwrap_or_else(|e| {
                        panic!("failed to restart child {pid} after suppressing SIGSTOP: {e}")
                    });
                    continue;
                }

                syscall(pid, signal).unwrap_or_else(|e| {
                    panic!("failed to restart child {pid} after signal {signal}: {e}")
                });
            }
            Ok(WaitStatus::PtraceEvent(pid, _, event))
                if event == Event::PTRACE_EVENT_EXEC as c_int =>
            {
                syscall(pid, None).unwrap_or_else(|e| {
                    panic!(
                        "failed to restart child {pid} after event {:?}: {e}",
                        event_from_int(event)
                    );
                });
            }
            Ok(WaitStatus::PtraceEvent(pid, _, event))
                if event == Event::PTRACE_EVENT_FORK as c_int
                    || event == Event::PTRACE_EVENT_VFORK as c_int
                    || event == Event::PTRACE_EVENT_CLONE as c_int =>
            {
                let new_child_pid = Pid::from_raw(
                    getevent(pid)
                        .unwrap_or_else(|e| panic!("failed to get new child of {pid}: {e}"))
                        .try_into()
                        .unwrap(),
                );
                if !ignore_next_stop.insert(new_child_pid) {
                    panic!("new child {new_child_pid} already in list to ignore next SIGSTOP");
                }
                syscall(pid, None).unwrap_or_else(|e| {
                    panic!(
                        "failed to restart child {pid} after event {:?}: {e}",
                        event_from_int(event)
                    );
                });
            }
            Ok(status) => panic!("unexpected child process status {status:?}"),
            Err(errno) => panic!("error from waitpid: {errno}"),
        }
    }
}

pub fn execute(path: &CStr, args: &[&CStr], env: &[&CStr], config: &Config) -> ChildExit {
    match unsafe { fork() } {
        Ok(ForkResult::Child) => child(path, args, env),
        Ok(ForkResult::Parent { child, .. }) => parent(child, config),
        Err(errno) => panic!("failed to fork: {}", errno),
    }
}
