pub use config::{Check, Config, ConfigEntry};
pub use map::MemoryMap;
use nix::{
    sys::{
        ptrace::{getregs, kill, read, setoptions, syscall, traceme, AddressType, Options},
        wait::{waitpid, WaitStatus},
    },
    unistd::{execve, fork, ForkResult, Pid},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, ffi::CStr};
use syscalls::Sysno;
mod config;
mod map;

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
        Options::PTRACE_O_EXITKILL.union(Options::PTRACE_O_TRACESYSGOOD),
    )
    .expect("failed to set ptrace options");

    let mut memory_map = MemoryMap::from_pid(child).unwrap();

    println!("Starting to watch child...");
    loop {
        syscall(child, None).expect("failed to restart child");
        match waitpid(child, None).expect("failed to get status from waitpid") {
            WaitStatus::Exited(_, code) => {
                return ChildExit::Exited(code);
            }
            WaitStatus::PtraceSyscall(pid) => {
                if let Some(exit) = handle_syscall(pid, config, &mut memory_map) {
                    kill(pid).expect("failed to kill child");
                    return exit;
                }
            }
            status => panic!("unexpected child process status {status:?}"),
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
