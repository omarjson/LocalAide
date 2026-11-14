//! Process lifecycle: one running copy, no orphaned engine processes, and
//! startup failures the user can see.

use std::process::{Child, Command};
use std::sync::OnceLock;

/// Holds the single-instance lock for as long as this process runs.
static INSTANCE_LOCK: OnceLock<std::fs::File> = OnceLock::new();

/// Takes the single-instance lock. Returns false when another copy holds it,
/// after telling the user. The OS releases the lock when a process ends, even
/// after a crash, so a leftover lock file never blocks a new start.
pub fn claim_single_instance() -> bool {
    let dir = crate::app::paths::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(dir.join("instance.lock"))
    {
        Ok(file) => file,
        Err(e) => {
            // Starting without the lock beats refusing to start at all.
            log::warn!("Could not open the instance lock file: {e}");
            return true;
        }
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = INSTANCE_LOCK.set(file);
            true
        }
        Err(std::fs::TryLockError::WouldBlock) => {
            show_message_box("LocalAide", "LocalAide is already open.");
            false
        }
        Err(std::fs::TryLockError::Error(e)) => {
            log::warn!("Could not lock the instance file: {e}");
            true
        }
    }
}

/// Stops a child process when LocalAide exits, even after a crash or a kill
/// from Task Manager. On Windows the child joins a job whose processes end when
/// the job's last handle closes. The OS closes that handle when LocalAide ends.
pub fn tie_child_to_app(child: &Child) {
    #[cfg(windows)]
    job::assign(child);
    #[cfg(not(windows))]
    let _ = child;
}

/// Keeps a console window from opening next to the app for a child process.
pub fn hide_console_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Reports a startup failure the user can see, then ends the process. This
/// replaces the old fallback to a temp folder, which would silently lose data
/// at the next restart.
pub fn fatal_startup_error(message: &str) -> ! {
    log::error!("{message}");
    show_message_box("LocalAide cannot start", message);
    std::process::exit(1)
}

#[cfg(windows)]
fn show_message_box(title: &str, message: &str) {
    use std::ffi::c_void;

    const MB_ICONERROR: u32 = 0x0000_0010;

    #[link(name = "user32")]
    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            style: u32,
        ) -> i32;
    }

    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let text = wide(message);
    let caption = wide(title);
    // SAFETY: both buffers end in a NUL and live until the call returns. A null
    // owner window makes the box a top-level window.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_message_box(_title: &str, message: &str) {
    eprintln!("{message}");
}

#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;

    // Layouts from the Win32 headers. The fields are written, never read.
    #[allow(dead_code)]
    #[repr(C)]
    struct IoCounters {
        read_operations: u64,
        write_operations: u64,
        other_operations: u64,
        read_transfer: u64,
        write_transfer: u64,
        other_transfer: u64,
    }

    #[allow(dead_code)]
    #[repr(C)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[allow(dead_code)]
    #[repr(C)]
    struct ExtendedLimitInformation {
        basic: BasicLimitInformation,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
        fn SetInformationJobObject(
            job: *mut c_void,
            class: i32,
            info: *const c_void,
            length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    }

    /// The job handle as an integer (0 when there is none), so it can sit in a
    /// static. It is never closed: the handle must stay open while the app runs.
    static KILL_ON_CLOSE_JOB: OnceLock<usize> = OnceLock::new();

    fn kill_on_close_job() -> usize {
        *KILL_ON_CLOSE_JOB.get_or_init(|| {
            // SAFETY: null arguments ask for an unnamed job with default security.
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return 0;
            }
            // SAFETY: an all-zero value is valid for this struct, which holds only integers.
            let mut info: ExtendedLimitInformation = unsafe { std::mem::zeroed() };
            info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `info` has the layout Windows expects and outlives the call.
            let set = unsafe {
                SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    (&info as *const ExtendedLimitInformation).cast(),
                    std::mem::size_of::<ExtendedLimitInformation>() as u32,
                )
            };
            if set == 0 {
                return 0;
            }
            job as usize
        })
    }

    /// Puts `child` into the kill-on-close job. A failure leaves the child
    /// untied, and the normal exit path still stops it.
    pub fn assign(child: &Child) {
        let job = kill_on_close_job();
        if job == 0 {
            return;
        }
        // SAFETY: the job handle comes from CreateJobObjectW and is never closed.
        // The child's handle stays valid while the child object is alive.
        let _ = unsafe { AssignProcessToJobObject(job as *mut c_void, child.as_raw_handle()) };
    }
}
