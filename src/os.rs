//! What differs between Unix and Windows: detaching the daemon, checking and ending
//! processes, executables, the shell for hooks and the Python interpreter for monitors.

#[cfg(unix)]
mod imp {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::path::Path;

    pub const PYTHON: &str = "python3";

    /// Run `cmd` in its own session so it outlives the terminal that started it.
    pub fn detach(cmd: &mut std::process::Command) {
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    pub fn alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    /// Start `cmd` as the leader of a new process group, so `end_group` reaches its children.
    pub fn new_group(cmd: &mut tokio::process::Command) {
        cmd.process_group(0);
    }

    pub fn end_group(pid: u32) {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }
    }

    pub fn shell(line: &str) -> tokio::process::Command {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(line);
        c
    }

    pub fn is_executable(p: &Path) -> bool {
        std::fs::metadata(p).map_or(false, |m| m.permissions().mode() & 0o111 != 0)
    }

    pub fn make_executable(p: &Path) -> std::io::Result<()> {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))
    }

    /// Resolves when the process is asked to stop (SIGTERM or SIGINT).
    pub async fn stop_requested() {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("SIGINT handler");
        tokio::select! { _ = term.recv() => {}, _ = int.recv() => {} }
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, SetHandleInformation, HANDLE_FLAG_INHERIT, STILL_ACTIVE};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS, PROCESS_QUERY_LIMITED_INFORMATION};

    pub const PYTHON: &str = "python";

    /// Run `cmd` without a console so it outlives the terminal that started it. Windows hands
    /// every inheritable handle to a child, so this process's own stdin, stdout and stderr stop
    /// being inheritable: otherwise the child holds a caller's pipe open and the caller waits forever.
    pub fn detach(cmd: &mut std::process::Command) {
        for h in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            unsafe {
                SetHandleInformation(GetStdHandle(h), HANDLE_FLAG_INHERIT, 0);
            }
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            ok && code == STILL_ACTIVE as u32
        }
    }

    pub fn new_group(cmd: &mut tokio::process::Command) {
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    /// End the process and every process it started.
    pub fn end_group(pid: u32) {
        let _ = std::process::Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(CREATE_NO_WINDOW).output();
    }

    pub fn shell(line: &str) -> tokio::process::Command {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").raw_arg(line).creation_flags(CREATE_NO_WINDOW);
        c
    }

    pub fn is_executable(p: &Path) -> bool {
        p.extension().and_then(|x| x.to_str()).map_or(false, |x| ["exe", "cmd", "bat"].contains(&x.to_ascii_lowercase().as_str()))
    }

    pub fn make_executable(_: &Path) -> std::io::Result<()> {
        Ok(())
    }

    /// Resolves on Ctrl+C. A detached daemon has no console, so it stops through its API.
    pub async fn stop_requested() {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

pub use imp::*;
