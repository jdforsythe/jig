// IMPORTANT: Never call process::exit() from this module.
// After SessionGuard is live, valid exits are:
//   - Normal return (runs Drop)
//   - exec (drop guard FIRST — exec does not run Drop)
//   - panic (panic hook runs Category A cleanup before abort)

use std::ffi::CString;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{debug, info};

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("claude binary not found in PATH or known locations")]
    ClaudeNotFound,

    #[error("Failed to exec claude: {0}")]
    ExecFailed(std::io::Error),

    #[error("Fork failed: {0}")]
    ForkFailed(nix::Error),

    #[error("Signal setup failed: {0}")]
    SignalSetup(#[from] std::io::Error),
}

/// Finds the claude binary in $PATH or known fallback locations.
pub fn find_claude_binary() -> Option<PathBuf> {
    // Search $PATH first
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join("claude");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Fallback locations
    let fallbacks = [
        home::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("local")
            .join("claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];

    for path in &fallbacks {
        if path.is_file() {
            tracing::warn!(
                "claude found outside PATH: {}. Consider adding to PATH.",
                path.display()
            );
            return Some(path.clone());
        }
    }

    None
}

/// Forks and execs `claude` with the given arguments.
/// The parent installs signal handlers and waits for the child.
/// Returns the child's exit code.
///
/// # Safety
/// Uses nix::unistd::fork. The child immediately calls exec.
/// IMPORTANT: Drop the fd-lock guard BEFORE calling this function.
pub fn fork_and_exec(claude_bin: &Path, args: &[String]) -> Result<i32, ExecutorError> {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::{ForkResult, Pid, execvp, fork, setpgid};
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    // Build CString args
    let prog = CString::new(claude_bin.to_string_lossy().as_bytes())
        .map_err(|e| ExecutorError::ExecFailed(std::io::Error::other(e.to_string())))?;

    let mut c_args: Vec<CString> = Vec::with_capacity(args.len() + 1);
    c_args.push(prog.clone());
    for arg in args {
        c_args.push(
            CString::new(arg.as_bytes())
                .map_err(|e| ExecutorError::ExecFailed(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?,
        );
    }

    // Register signal handlers BEFORE fork
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;

    info!("Forking claude: {} {:?}", claude_bin.display(), args);

    // Fork
    // SAFETY: We immediately exec in the child. No Rust code runs after fork in the child
    // except setpgid and execvp. We use _exit on exec failure.
    let fork_result = unsafe { fork() }.map_err(ExecutorError::ForkFailed)?;

    match fork_result {
        ForkResult::Child => {
            // Create new process group so killpg reaches all grandchildren
            let _ = setpgid(Pid::from_raw(0), Pid::from_raw(0));

            // Execute claude — replaces this process image
            let _ = execvp(&prog, &c_args);

            // exec failed — use _exit to avoid running atexit handlers in child
            // SAFETY: _exit is async-signal-safe; does not flush stdio
            unsafe { libc::_exit(127) };
            // SAFETY: _exit never returns — this is unreachable
            #[allow(unreachable_code)]
            return Err(ExecutorError::ExecFailed(
                std::io::Error::other("unreachable"),
            ));
        }
        ForkResult::Parent { child } => {
            let child_pgid = child; // pgid == child pid after setpgid(0,0)

            // Forward signals to the entire process group
            std::thread::spawn(move || {
                for sig in signals.forever() {
                    debug!("Forwarding signal {} to child pgid {}", sig, child_pgid);
                    let _ = nix::sys::signal::killpg(
                        child_pgid,
                        nix::sys::signal::Signal::try_from(sig).unwrap_or(nix::sys::signal::Signal::SIGTERM),
                    );
                }
            });

            // waitpid loop with EINTR retry
            loop {
                match waitpid(child, Some(WaitPidFlag::empty())) {
                    Ok(WaitStatus::Exited(_, code)) => {
                        info!("Child exited with code {}", code);
                        return Ok(code);
                    }
                    Ok(WaitStatus::Signaled(_, sig, _)) => {
                        info!("Child killed by signal {:?}", sig);
                        // Re-raise so jig's exit code reflects the signal
                        unsafe { libc::raise(sig as libc::c_int) };
                        return Ok(128 + sig as i32);
                    }
                    Err(nix::errno::Errno::EINTR) => continue, // retry on EINTR (macOS)
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::warn!("waitpid error: {}", e);
                        return Ok(1);
                    }
                }
            }
        }
    }
}
