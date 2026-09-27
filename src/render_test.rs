use std::{
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

static RENDER: Mutex<()> = Mutex::new(());

pub struct Guard {
    stop: Option<mpsc::Sender<()>>,
    watchdog: Option<JoinHandle<()>>,
    _render: MutexGuard<'static, ()>,
}

pub fn lock() -> Guard {
    let render = RENDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let seconds = include_str!("../web/render-timeout-seconds")
        .trim()
        .parse::<u64>()
        .unwrap();
    let name = thread::current().name().unwrap_or("render").to_owned();
    #[cfg(target_os = "linux")]
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_PTRACER, libc::PR_SET_PTRACER_ANY, 0, 0, 0) },
        0,
        "allow the timeout debugger to capture render threads"
    );
    #[cfg(target_os = "linux")]
    let tid = unsafe { libc::syscall(libc::SYS_gettid) };
    let (stop, done) = mpsc::channel();
    let watchdog = thread::spawn(move || {
        if done.recv_timeout(Duration::from_secs(seconds)) == Err(mpsc::RecvTimeoutError::Timeout) {
            let mut stderr = std::io::stderr().lock();
            use std::io::Write;
            let _ = writeln!(
                stderr,
                "render test {name} exceeded {seconds} s; stuck thread backtraces:"
            );
            drop(stderr);
            let mut debugger = Command::new("timeout");
            debugger.args([
                "--signal=KILL",
                "15s",
                "gdb",
                "--batch",
                "--nx",
                "--quiet",
                "-ex",
                "set print frame-arguments none",
            ]);
            #[cfg(target_os = "linux")]
            debugger.args(["-ex", &format!("python next(t for t in gdb.selected_inferior().threads() if t.ptid[1] == {tid}).switch()"), "-ex", "bt"]);
            let status = debugger
                .args(["-ex", "set pagination off", "-ex", "thread apply all bt"])
                .args(["--pid", &std::process::id().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            eprintln!("timeout debugger: {status:?}");
            std::process::exit(1);
        }
    });
    Guard {
        stop: Some(stop),
        watchdog: Some(watchdog),
        _render: render,
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(watchdog) = self.watchdog.take() {
            watchdog.join().unwrap();
        }
    }
}
