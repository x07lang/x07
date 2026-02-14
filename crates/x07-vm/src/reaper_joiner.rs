use std::process::Child;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;

static TX: OnceLock<mpsc::Sender<Child>> = OnceLock::new();

pub fn register(child: Child) {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Child>();

        let _ = thread::Builder::new()
            .name("x07-vm-reaper-joiner".to_string())
            .spawn(move || joiner_loop(rx));

        tx
    });

    match tx.send(child) {
        Ok(()) => {}
        Err(mpsc::SendError(child)) => spawn_fallback_waiter(child),
    }
}

fn joiner_loop(rx: mpsc::Receiver<Child>) {
    let mut children: Vec<Child> = Vec::new();
    let mut rx_open = true;

    let tick = Duration::from_millis(250);

    while rx_open || !children.is_empty() {
        match rx.recv_timeout(tick) {
            Ok(ch) => children.push(ch),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => rx_open = false,
        }

        children.retain_mut(|child| match child.try_wait() {
            Ok(Some(_status)) => false,
            Ok(None) => true,
            Err(err) => {
                #[cfg(unix)]
                {
                    if err.raw_os_error() == Some(libc::ECHILD) {
                        return false;
                    }
                }
                true
            }
        });
    }
}

fn spawn_fallback_waiter(mut child: Child) {
    let _ = thread::Builder::new()
        .name("x07-vm-reaper-wait".to_string())
        .spawn(move || {
            let _ = child.wait();
        });
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::register;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[test]
    fn joiner_reaps_quick_exit_child_no_zombie_left() {
        let child = Command::new("true")
            .spawn()
            .or_else(|_| Command::new("sh").args(["-c", "exit 0"]).spawn())
            .expect("spawn quick-exit child");

        let pid: libc::pid_t = child.id().try_into().expect("pid_t conversion");

        register(child);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut saw_esrch = false;
        while Instant::now() < deadline {
            let r = unsafe { libc::kill(pid, 0) };
            if r == -1 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() == Some(libc::ESRCH) {
                    saw_esrch = true;
                    break;
                }
            }
            sleep(Duration::from_millis(10));
        }

        if !saw_esrch {
            let mut status: libc::c_int = 0;
            let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            if r == -1 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() == Some(libc::ECHILD) {
                    return;
                }
            }
            panic!("child pid did not disappear (possible zombie or pid reuse): {pid}");
        }

        let mut status: libc::c_int = 0;
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        assert_eq!(r, -1);
        let e = std::io::Error::last_os_error();
        assert_eq!(e.raw_os_error(), Some(libc::ECHILD));
    }
}
