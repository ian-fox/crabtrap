pub use config::{Config, ConfigEntry};
use nix::{
    sys::{
        ptrace::{setoptions, syscall, traceme, Options},
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

/// parent attaches to the child with ptrace and then watches for syscalls in a loop
fn parent(child: Pid, _config: &Config) -> ChildExit {
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
            WaitStatus::PtraceSyscall(_pid) => {
                // This is where the syscall handling logic will go.
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
