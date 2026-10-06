use process_wrap::std::{ChildWrapper, CommandWrap};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

pub type Cancellation = Arc<AtomicUsize>;

type Output = (bool, Vec<u8>);

// One bounded, best-effort console worker for the whole invocation. A stalled
// collector can block this worker, never lifecycle polling or durable reports.
pub struct Console {
    sender: SyncSender<Output>,
    finished: Receiver<()>,
}

impl Console {
    pub fn new() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Output>(16);
        let (finished_tx, finished) = mpsc::sync_channel(1);
        thread::Builder::new().name("console-forwarder".into()).spawn(move || {
            // Keep the stdout lock until this thread exits, including while
            // blocked. Rust's shutdown try_lock then cannot flush it on the
            // supervisor thread. No runtime status writes bypass this worker.
            let mut stdout = io::stdout().lock();
            for (error, bytes) in receiver {
                let sink: &mut dyn Write = if error { &mut io::stderr() } else { &mut stdout };
                let _ = sink.write_all(&bytes).and_then(|()| sink.flush());
            }
            let _ = finished_tx.send(());
        })?;
        Ok(Self { sender, finished })
    }

    pub fn send(&self, error: bool, bytes: &[u8]) {
        for chunk in bytes.chunks(8192) {
            if self.sender.try_send((error, chunk.to_vec())).is_err() {
                break; // The full stage output remains in its file.
            }
        }
    }

    pub fn finish(self) {
        drop(self.sender);
        // Never join a worker that might be blocked in an OS console/pipe write.
        let _ = self.finished.recv_timeout(Duration::from_millis(100));
    }
}

pub fn install_handlers() -> io::Result<Cancellation> {
    let cancelled = Arc::new(AtomicUsize::new(0));
    #[cfg(unix)]
    for signal in
        [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP]
    {
        signal_hook::flag::register_usize(signal, cancelled.clone(), (128 + signal) as usize)?;
    }
    #[cfg(windows)]
    {
        let flag = cancelled.clone();
        ctrlc::set_handler(move || {
            flag.store(130, Ordering::SeqCst);
        })
        .map_err(io::Error::other)?;
        // A launcher may pass down the Windows "ignore Ctrl+C" attribute.
        // Registering a handler alone does not clear that inherited attribute.
        // SAFETY: null callback toggles the documented console-process attribute.
        if unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        own_windows_descendants()?;
    }
    Ok(cancelled)
}

// A process-lifetime job also covers abrupt native termination (including Git Bash
// kill, which does not deliver a Unix signal to an MSVC executable). Per-command
// jobs below handle ordinary success, failure and cancellation. No inherited job
// handle can keep this outer job alive after the runner dies.
#[cfg(windows)]
fn own_windows_descendants() -> io::Result<()> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::GetCurrentProcess,
        },
    };
    // SAFETY: valid local structures/handles, no inheritable security attributes.
    // The handle deliberately lives until OS process teardown: closing a job
    // containing this process earlier would terminate us before returning a code.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(job, GetCurrentProcess()) == 0
        {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(error);
        }
    }
    Ok(())
}

pub fn exit_code(status: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
    }
    #[cfg(not(unix))]
    {
        status.code().unwrap_or(1)
    }
}

pub struct OwnedProcess(Box<dyn ChildWrapper>, bool);

impl OwnedProcess {
    pub fn spawn(mut command: Command, log: &Path) -> io::Result<Self> {
        // Windows append-only handles make Git Bash/MSYS exit silently with 1.
        // Keep a normal writable handle and seek once; stdout/stderr clones share
        // its cursor, and the serial runner never writes it while the child runs.
        let mut file = OpenOptions::new().create(true).write(true).truncate(false).open(log)?;
        file.seek(SeekFrom::End(0))?;
        command.stdin(Stdio::null()).stdout(file.try_clone()?).stderr(file);
        let mut wrapped = CommandWrap::from(command);
        #[cfg(unix)]
        wrapped.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(process_wrap::std::JobObject);
        wrapped.spawn().map(|child| Self(child, false))
    }

    pub fn id(&self) -> u32 {
        self.0.id()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        // Observe the stage leader, even if a broken script leaves grandchildren.
        // Cleanup below then stops those descendants before the next stage starts.
        // SAFETY: the sole wrapper is ProcessGroup/JobObject. We only cache the
        // leader's exit in std::Child; we never replace it or bypass group cleanup.
        // Wrapper wait() subsequently reads that cached status and reaps the group.
        unsafe { self.0.try_inner_child_mut() }.expect("native command").try_wait()
    }

    pub fn stop(&mut self, code: usize) -> io::Result<()> {
        if self.1 {
            return Ok(());
        }
        #[cfg(unix)]
        {
            let signal = if code == 0 { signal_hook::consts::SIGTERM } else { (code - 128) as i32 };
            if self.0.signal(signal).is_ok() {
                // Give existing script traps and browser cleanup a bounded grace.
                let deadline = std::time::Instant::now() + Duration::from_millis(1500);
                loop {
                    // Reap the leader before probing: a zombie still keeps its
                    // process group alive. The leader exiting alone is NOT proof
                    // that its children (possibly ignoring TERM) have exited.
                    self.try_wait()?;
                    // SAFETY: spawn created a dedicated group led by this PID.
                    // Signal zero probes every member without sending a signal.
                    let exists = unsafe { libc::kill(-(self.id() as libc::pid_t), 0) };
                    if exists == -1
                        && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
                    {
                        self.0.wait()?;
                        self.1 = true;
                        return Ok(());
                    }
                    // Any other probe result is inconclusive: retain the grace
                    // and forced group cleanup, including when the leader exited.
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
        #[cfg(windows)]
        if code != 0 {
            thread::sleep(Duration::from_millis(300));
        }
        let killed = self.0.start_kill();
        // ESRCH means the Unix group already exited. Other cleanup failures matter.
        #[cfg(unix)]
        if let Err(error) = killed {
            if error.raw_os_error() == Some(libc::ESRCH) {
                self.0.wait()?;
                self.1 = true;
                return Ok(());
            }
            return Err(error);
        }
        #[cfg(not(unix))]
        killed?;
        self.0.wait()?;
        self.1 = true;
        Ok(())
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        // Backstop for log/report I/O errors and Rust unwinding.
        if !self.1 {
            let _ = self.0.start_kill();
            let _ = self.0.wait();
        }
    }
}

pub struct LogTail<'a>(File, &'a Console);
impl<'a> LogTail<'a> {
    pub fn open(path: &Path, console: &'a Console) -> io::Result<Self> {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::End(0))?;
        Ok(Self(file, console))
    }
    pub fn drain(&mut self) -> io::Result<()> {
        let mut bytes = [0; 8192];
        // Also bound file reads: a continuously noisy stage must not starve polling.
        for _ in 0..16 {
            let count = self.0.read(&mut bytes)?;
            if count == 0 {
                return Ok(());
            }
            // Losing the console must not lose the durable stage log or its status.
            self.1.send(false, &bytes[..count]);
        }
        Ok(())
    }
}

pub fn wait(
    child: &mut OwnedProcess,
    tail: &mut LogTail,
    cancelled: &Cancellation,
) -> io::Result<i32> {
    loop {
        let code = cancelled.load(Ordering::SeqCst);
        if code != 0 {
            child.stop(code)?;
            tail.drain()?;
            return Ok(code as i32);
        }
        tail.drain()?;
        if let Some(status) = child.try_wait()? {
            child.stop(0)?;
            tail.drain()?;
            return Ok(exit_code(status));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

#[cfg(all(test, unix))]
mod cleanup_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn completed_group_does_not_consume_grace() {
        let log =
            std::env::temp_dir().join(format!("release-ui-cleanup-{}.log", std::process::id()));
        let mut command = Command::new("sleep");
        command.arg("30");
        let mut child = OwnedProcess::spawn(command, &log).unwrap();
        let started = Instant::now();
        child.stop(0).unwrap();
        assert!(started.elapsed() < Duration::from_millis(1200), "empty group consumed grace");
        assert!(child.try_wait().unwrap().is_some(), "leader was not reaped");
        child.stop(0).unwrap(); // Already stopped: safe and idempotent.
        std::fs::remove_file(log).unwrap();
    }
}
