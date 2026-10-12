//! Ownership of an engine's process group/job and cancellation of pipe workers.
use std::{
    io,
    process::{Child, Command},
    thread::JoinHandle,
};

pub(crate) struct ProcessTree {
    #[cfg(unix)]
    group: libc::pid_t,
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
}
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            let child = command.spawn()?;
            let tree = Self {
                group: child.id() as libc::pid_t,
            };
            Ok((child, tree))
        }
        #[cfg(windows)]
        {
            use std::os::windows::{
                io::{AsRawHandle, FromRawHandle, OwnedHandle},
                process::CommandExt,
            };
            use windows_sys::Win32::System::JobObjects::*;
            // Create/configure the job before running any engine code.
            let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if unsafe {
                SetInformationJobObject(
                    raw,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const std::ffi::c_void,
                    std::mem::size_of_val(&limits) as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Suspend until job assignment so a fast wrapper cannot spawn a
            // descendant outside the owned job before assignment completes.
            command.creation_flags(0x08000000 | 0x00000004); // NO_WINDOW | SUSPENDED
            let mut child = command.spawn()?;
            let assigned = (|| {
                if unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                resume_initial_thread(child.id())
            })();
            if let Err(error) = assigned {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok((child, Self { job }))
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok((command.spawn()?, Self {}))
        }
    }
    pub fn terminate(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            // The group remains owned even after the direct wrapper exits.
            if unsafe { libc::kill(-self.group, libc::SIGKILL) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            if unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(
                    self.job.as_raw_handle(),
                    1,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
pub(crate) fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(windows)]
fn resume_initial_thread(pid: u32) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of_val(&entry) as u32;
    if unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } == 0 {
        return Err(io::Error::last_os_error());
    }
    loop {
        if entry.th32OwnerProcessID == pid {
            let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            return Ok(());
        }
        if unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Suspended engine thread not found",
            ));
        }
    }
}
#[cfg(not(unix))]
pub(crate) fn nonblocking<T>(_pipe: &T) -> io::Result<()> {
    Ok(())
}

pub(crate) fn cancel_blocked<T>(_worker: &JoinHandle<T>) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        // Repeated while joining to cover a read starting just after cancellation.
        unsafe {
            windows_sys::Win32::System::IO::CancelSynchronousIo(_worker.as_raw_handle());
        }
    }
}
