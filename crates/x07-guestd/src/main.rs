#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("x07-guestd is only supported on Linux guests");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    let code = linux::run();
    unsafe { libc::_exit(code) }
}

#[cfg(target_os = "linux")]
mod linux;
