//! Minimal POSIX FFI for Linux & macOS (zero external crates).

#![cfg(unix)]

use std::os::raw::{c_double, c_int, c_ulong, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

const POLLIN: i16 = 1;
pub const SIGWINCH: c_int = 28;
pub const SIGTERM: c_int = 15;
pub const SIGINT: c_int = 2;
const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;

#[cfg(target_os = "linux")]
const TIOCGWINSZ: c_ulong = 0x5413;
#[cfg(target_os = "macos")]
const TIOCGWINSZ: c_ulong = 0x40087468;

#[cfg(target_os = "linux")]
const O_NONBLOCK: c_int = 0o4000;
#[cfg(target_os = "macos")]
const O_NONBLOCK: c_int = 0x0004;

extern "C" {
    fn isatty(fd: c_int) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn getloadavg(loadavg: *mut c_double, nelem: c_int) -> c_int;
    fn pipe(fds: *mut c_int) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn close(fd: c_int) -> c_int;
    fn poll(fds: *mut PollFd, nfds: c_ulong, timeout: c_int) -> c_int;
    fn signal(sig: c_int, handler: usize) -> usize;
}

pub fn stdout_isatty() -> bool {
    unsafe { isatty(1) != 0 }
}

pub fn terminal_size() -> Option<(usize, usize)> {
    if !stdout_isatty() {
        return None;
    }
    let mut ws = WinSize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let rc = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut WinSize) };
    if rc == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
        Some((ws.ws_col as usize, ws.ws_row as usize))
    } else {
        None
    }
}

pub fn loadavg() -> Option<(f64, f64, f64)> {
    let mut avg = [0.0f64; 3];
    let n = unsafe { getloadavg(avg.as_mut_ptr(), 3) };
    if n == 3 {
        Some((avg[0], avg[1], avg[2]))
    } else {
        None
    }
}

static WAKE_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static TERM_SIGNALLED: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_winch(_sig: c_int) {
    let fd = WAKE_WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let b: u8 = 1;
        unsafe {
            let _ = write(fd, &b as *const u8 as *const c_void, 1);
        }
    }
}

extern "C" fn on_terminate(sig: c_int) {
    TERM_SIGNALLED.store(sig, Ordering::SeqCst);
}

pub fn install_term_handlers() {
    unsafe {
        signal(SIGINT, on_terminate as *const () as usize);
        signal(SIGTERM, on_terminate as *const () as usize);
    }
}

pub fn terminated() -> bool {
    TERM_SIGNALLED.load(Ordering::SeqCst) != 0
}

pub struct ResizeWaiter {
    read_fd: c_int,
    write_fd: c_int,
}

impl ResizeWaiter {
    pub fn new() -> Option<Self> {
        let mut fds = [0 as c_int; 2];
        if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
            return None;
        }
        unsafe {
            let flags = fcntl(fds[1], F_GETFL);
            if flags >= 0 {
                fcntl(fds[1], F_SETFL, flags | O_NONBLOCK);
            }
            let rflags = fcntl(fds[0], F_GETFL);
            if rflags >= 0 {
                fcntl(fds[0], F_SETFL, rflags | O_NONBLOCK);
            }
        }
        WAKE_WRITE_FD.store(fds[1], Ordering::SeqCst);
        unsafe {
            signal(SIGWINCH, on_winch as *const () as usize);
        }
        Some(Self {
            read_fd: fds[0],
            write_fd: fds[1],
        })
    }

    /// Wait up to `timeout_ms` milliseconds, returning early on SIGWINCH resize or signal.
    pub fn wait_ms(&self, timeout_ms: i32) {
        let mut pfd = PollFd {
            fd: self.read_fd,
            events: POLLIN,
            revents: 0,
        };
        let rc = unsafe { poll(&mut pfd as *mut PollFd, 1, timeout_ms) };
        if rc > 0 && (pfd.revents & POLLIN) != 0 {
            let mut buf = [0u8; 256];
            unsafe {
                while read(self.read_fd, buf.as_mut_ptr() as *mut c_void, buf.len()) > 0 {}
            }
        }
    }
}

impl Drop for ResizeWaiter {
    fn drop(&mut self) {
        WAKE_WRITE_FD.store(-1, Ordering::SeqCst);
        unsafe {
            close(self.read_fd);
            close(self.write_fd);
        }
    }
}
