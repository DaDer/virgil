//! Process liveness checks (contract 2.2).

/// Answers whether a process with this pid exists. Tests inject their own.
pub trait PidProbe: Send + Sync {
    fn is_alive(&self, pid: u32) -> bool;
}

/// The operating system's answer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPidProbe;

impl PidProbe for SystemPidProbe {
    fn is_alive(&self, pid: u32) -> bool {
        pid_alive(pid)
    }
}

/// A probe with fixed answers, for tests and fixtures.
#[derive(Debug, Default, Clone)]
pub struct FixedPidProbe {
    pub alive: Vec<u32>,
}

impl PidProbe for FixedPidProbe {
    fn is_alive(&self, pid: u32) -> bool {
        self.alive.contains(&pid)
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    // SAFETY: kill with signal 0 only checks for existence and permission; it sends nothing.
    let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if r == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    if pid == 0 || pid % 4 != 0 {
        // Windows pids are multiples of 4.
        return false;
    }
    // SAFETY: plain Win32 calls; the handle is closed before returning.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        !ok || code == STILL_ACTIVE as u32
    }
}

#[cfg(not(any(unix, windows)))]
fn pid_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive_and_huge_pid_is_not() {
        assert!(SystemPidProbe.is_alive(std::process::id()));
        assert!(!SystemPidProbe.is_alive(99_999_999));
    }
}
