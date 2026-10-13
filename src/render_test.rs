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
            #[cfg(target_os = "linux")]
            let mut debugger = debugger(std::process::id(), tid, "15s");
            #[cfg(not(target_os = "linux"))]
            let mut debugger = debugger(std::process::id(), "15s");
            let status = debugger
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

/// A debugger killed while a thread waits uninterruptibly leaves that thread's attach
/// SIGSTOP pending, stopping the whole process once the wait returns.
fn debugger(pid: u32, #[cfg(target_os = "linux")] tid: libc::c_long, kill_after: &str) -> Command {
    let mut debugger = Command::new("sh");
    debugger.args([
        "-c",
        &format!("timeout --signal=KILL {kill_after} \"$@\"; s=$?; kill -CONT {pid}; exit $s"),
        "sh",
        "gdb",
        "--batch",
        "--nx",
        "--quiet",
        "-ex",
        "set print frame-arguments none",
    ]);
    #[cfg(target_os = "linux")]
    debugger.args(["-ex", &format!("python next(t for t in gdb.selected_inferior().threads() if t.ptid[1] == {tid}).switch()"), "-ex", "bt"]);
    #[cfg(not(target_os = "linux"))]
    debugger.args(["-ex", "thread apply all bt"]);
    debugger.args(["--pid", &pid.to_string()]);
    debugger
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(watchdog) = self.watchdog.take() {
            watchdog.join().unwrap();
        }
    }
}

#[test]
fn error_exits_report_the_requesting_renderer_and_cause() {
    use bevy::{
        app::{App, AppExit, SubApp},
        ecs::schedule::ScheduleLabel,
        prelude::*,
        render::{
            RenderApp,
            error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
            pipelined_rendering::PipelinedRenderingPlugin,
            renderer::WgpuWrapper,
        },
    };

    if let Ok(case) = std::env::var("ZIRAL_EXIT_DIAGNOSTIC_CASE") {
        let mut app = App::new();
        if case == "device" {
            app.add_message::<AppExit>();
            let error = RenderError {
                ty: ErrorType::OutOfMemory,
                description: "diagnostic allocation".into(),
                source: Some(WgpuWrapper::new(Box::new(std::io::Error::other(
                    "Device::create_texture diagnostic source",
                )))),
            };
            let policy =
                (RenderErrorHandler::default().0)(&error, app.world_mut(), &mut World::new());
            assert!(matches!(policy, RenderErrorPolicy::StopRendering));
        } else {
            assert_eq!(case, "channel");
            app.add_plugins(MinimalPlugins);
            let mut render_app = SubApp::new();
            render_app.update_schedule = Some(Update.intern());
            render_app.add_systems(Update, || panic!("diagnostic render thread panic"));
            app.insert_sub_app(RenderApp, render_app);
            app.add_plugins(PipelinedRenderingPlugin);
            app.finish();
            app.cleanup();
            app.update();
            app.update();
        }
        assert_eq!(app.should_exit(), Some(AppExit::error()));
        return;
    }

    for (case, expected) in [
        (
            "device",
            vec![
                "RenderErrorHandler requested AppExit::Error",
                "OutOfMemory",
                "diagnostic allocation",
                "Device::create_texture diagnostic source",
            ],
        ),
        (
            "channel",
            vec![
                "renderer_extract requested AppExit::Error",
                "render thread channel disconnected",
                "diagnostic render thread panic",
            ],
        ),
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "render_test::error_exits_report_the_requesting_renderer_and_cause",
                "--nocapture",
            ])
            .env("ZIRAL_EXIT_DIAGNOSTIC_CASE", case)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{case}: {stderr}");
        for text in expected {
            assert!(stderr.contains(text), "{case} omitted {text:?}: {stderr}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_debugger_killed_during_an_uninterruptible_wait_leaves_the_process_running() {
    if std::env::var_os("ZIRAL_UNINTERRUPTIBLE_WAIT_CASE").is_some() {
        unsafe {
            assert_eq!(
                libc::prctl(libc::PR_SET_PTRACER, libc::PR_SET_PTRACER_ANY, 0, 0, 0),
                0
            );
            if libc::syscall(
                libc::SYS_clone,
                (libc::CLONE_VFORK | libc::SIGCHLD) as libc::c_ulong,
                0,
                0,
                0,
                0,
            ) == 0
            {
                libc::sleep(6);
                libc::_exit(0);
            }
        }
        return;
    }
    let mut waiting = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "render_test::a_debugger_killed_during_an_uninterruptible_wait_leaves_the_process_running",
        ])
        .env("ZIRAL_UNINTERRUPTIBLE_WAIT_CASE", "1")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let pid = waiting.id();
    let tasks = format!("/proc/{pid}/task");
    let in_wait = || {
        std::fs::read_dir(&tasks)
            .into_iter()
            .flatten()
            .flatten()
            .any(|task| {
                std::fs::read_to_string(task.path().join("stat")).is_ok_and(|stat| {
                    stat.rsplit_once(')')
                        .is_some_and(|(_, rest)| rest.trim_start().starts_with('D'))
                })
            })
    };
    let start = std::time::Instant::now();
    while !in_wait() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the child never entered its vfork wait"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let killed = debugger(pid, pid as libc::c_long, "1s")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(killed.code(), Some(137), "the debugger must die mid-attach");
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = waiting.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(20) {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            waiting.kill().unwrap();
            waiting.wait().unwrap();
            panic!("the process never resumed: {stat}");
        }
        thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status}");
}
