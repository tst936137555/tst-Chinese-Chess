use super::*;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub(super) struct Process {
    child: std::process::Child,
    input: mpsc::SyncSender<(String, mpsc::Sender<Result<(), String>>)>,
    writer: Option<std::thread::JoinHandle<()>>,
    rx: Receiver<String>,
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Process {
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new(path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = path.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        Self::spawn(cmd)
    }
    fn spawn(mut cmd: std::process::Command) -> Result<Self, String> {
        use std::process::Stdio;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("启动引擎失败：{e}"))?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                loop {
                    let mut bytes = Vec::new();
                    match reader.by_ref().take(4097).read_until(b'\n', &mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if bytes.len() > 4096 => break,
                        Ok(_) => crate::diagnostics::record(
                            "engine-stderr",
                            &String::from_utf8_lossy(&bytes),
                        ),
                    }
                }
            });
        }
        let mut stdin = child.stdin.take().ok_or("引擎输入不可用")?;
        let (input, writes) = mpsc::sync_channel::<(String, mpsc::Sender<Result<(), String>>)>(1);
        let writer = std::thread::spawn(move || {
            while let Ok((line, done)) = writes.recv() {
                let result = writeln!(stdin, "{line}")
                    .and_then(|()| stdin.flush())
                    .map_err(|e| format!("引擎写入失败：{e}"));
                let failed = result.is_err();
                let _ = done.send(result);
                if failed {
                    break;
                }
            }
        });
        let output = child.stdout.take().ok_or("引擎输出不可用")?;
        let (tx, rx) = mpsc::sync_channel(256);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                match reader.by_ref().take(65_537).read_until(b'\n', &mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if bytes.len() > 65_536 => break,
                    Ok(_) => {
                        if tx
                            .send(String::from_utf8_lossy(&bytes).trim_end().to_string())
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            writer: Some(writer),
            rx,
        })
    }
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Transport for Process {
    fn send(&mut self, line: &str) -> Result<(), String> {
        self.send_until(
            line,
            Instant::now() + Duration::from_secs(5),
            &AtomicU64::new(0),
            0,
        )
    }
    fn send_until(
        &mut self,
        line: &str,
        deadline: Instant,
        token: &AtomicU64,
        id: u64,
    ) -> Result<(), String> {
        let (done, result) = mpsc::channel();
        self.input
            .try_send((line.into(), done))
            .map_err(|_| "引擎写入队列不可用")?;
        loop {
            if crate::platform::suspended() || token.load(Ordering::Acquire) != id {
                return Err("搜索已取消".into());
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("引擎写入超时")?;
            match result.recv_timeout(remaining.min(Duration::from_millis(40))) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("引擎写入线程已退出".into()),
            }
        }
    }
    fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String> {
        match self.rx.recv_timeout(timeout) {
            Ok(line) => Ok(Some(line)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err("引擎已退出".into()),
        }
    }
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Closing the sender wakes an idle writer; killing the child unblocks a full pipe.
        let (replacement, _) = mpsc::sync_channel(1);
        drop(std::mem::replace(&mut self.input, replacement));
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod process_tests {
    use super::*;
    #[test]
    #[ignore = "subprocess fixture, invoked only by pipe failure tests"]
    fn idle_child() {
        let mode = std::env::var("XIANGQI_FAULT_CHILD").expect("only launched by parent");
        println!("XIANGQI_FAULT_READY:{mode}");
        std::io::stdout().flush().unwrap();
        if mode == "exit" {
            std::process::exit(7);
        }
        if mode == "half" {
            print!("uci");
            std::io::stdout().flush().unwrap();
        }
        if mode == "noise" {
            loop {
                println!("info string still alive");
                std::io::stdout().flush().unwrap();
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        std::thread::sleep(Duration::from_secs(30));
    }
    fn child(mode: &str) -> Process {
        let test = format!("{}::idle_child", module_path!().split_once("::").unwrap().1);
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env("XIANGQI_FAULT_CHILD", mode);
        let mut child = Process::spawn(command).unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                Instant::now() < end,
                "fixture never entered fault mode {mode}"
            );
            if let Some(line) = child
                .receive(Duration::from_millis(40))
                .expect("fixture exited before acknowledging fault mode")
                && line == format!("XIANGQI_FAULT_READY:{mode}")
            {
                return child;
            }
        }
    }
    #[test]
    fn full_stdin_pipe_obeys_deadline_and_cancellation() {
        for cancel in [false, true] {
            let mut process = child("idle");
            let token = Arc::new(AtomicU64::new(0));
            let stop = token.clone();
            let stopper = std::thread::spawn(move || {
                if cancel {
                    std::thread::sleep(Duration::from_millis(80));
                    stop.store(1, Ordering::Release);
                }
            });
            let start = Instant::now();
            let result = process.send_until(
                &"x".repeat(2 * 1024 * 1024),
                start + Duration::from_millis(300),
                &token,
                0,
            );
            assert!(result.is_err());
            drop(process);
            stopper.join().unwrap();
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
    #[test]
    fn handshake_exit_partial_line_and_noise_share_one_deadline() {
        for mode in ["exit", "half", "noise"] {
            let mut session = Session::with_transport(Box::new(child(mode)));
            let start = Instant::now();
            session.deadline = Some(start + Duration::from_millis(300));
            assert!(
                session
                    .handshake(Path::new("test.nnue"), &AtomicU64::new(0), 0)
                    .is_err()
            );
            drop(session);
            if mode != "exit" {
                assert!(start.elapsed() >= Duration::from_millis(250));
            }
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
}
