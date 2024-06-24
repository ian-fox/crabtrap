pub use config::{Config, ConfigEntry};
use nix::{
    sys::{
        ptrace::{getregs, read, setoptions, syscall, traceme, AddressType, Options},
        wait::{waitpid, WaitStatus},
    },
    unistd::{execve, fork, ForkResult, Pid},
};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use syscalls::Sysno;
mod config;

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

/// handle_syscall walks up the stack to see where a syscall came from.
///
/// Reference: https://github.com/ARM-software/abi-aa/blob/2a70c42d62e9c3eb5887fa50b71257f20daca6f9/aapcs64/aapcs64.rst#646the-frame-pointer
fn handle_syscall(pid: Pid, _config: &Config) {
    let regs = getregs(pid).expect("failed to get registers");
    let syscall = Sysno::from(regs.regs[8] as u32);

    println!("Syscall: {syscall}");

    let mut frame_pointer: u64 = regs.regs[29];
    println!(
        "Initial pc: {pc:x}, lr: {lr:x}, fp: {frame_pointer:x}",
        pc = regs.pc,
        lr = regs.regs[30]
    );

    let mut saved_lr;
    loop {
        if frame_pointer == 0 {
            break;
        }

        saved_lr =
            read(pid, (frame_pointer + 8) as AddressType).expect("failed to read saved lr") as u64;

        println!("saved_lr: {saved_lr:x}, frame pointer: {frame_pointer:x}");

        frame_pointer =
            read(pid, frame_pointer as AddressType).expect("failed to read frame pointer") as u64;
    }

    println!("Bottom of stack.");
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

    println!("Starting to watch child...");
    loop {
        syscall(child, None).expect("failed to restart child");
        match waitpid(child, None).expect("failed to get status from waitpid") {
            WaitStatus::Exited(_, code) => {
                return ChildExit::Exited(code);
            }
            WaitStatus::PtraceSyscall(pid) => {
                handle_syscall(pid, config);
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
