use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::logger;
use crate::vits::protocol::{VitsPingResponse, VitsRequest, VitsResponse};

/// 应用级共享状态：全局唯一的 VITS 运行时实例
pub type VitsState = Arc<Mutex<VitsRuntime>>;

pub fn create_vits_state(data_dir: &Path) -> VitsState {
    Arc::new(Mutex::new(VitsRuntime::new(data_dir)))
}

/// 指定数据目录下 VITS 运行时 exe 的路径
pub fn runtime_exe_path(data_dir: &Path) -> PathBuf {
    data_dir.join("vits_runtime").join("vits_runtime.exe")
}

/// 管理持久化的 VITS Python 子进程，通过 stdin/stdout 逐行 JSON-RPC 交互
pub struct VitsRuntime {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    exe_path: PathBuf,
}

impl VitsRuntime {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            child: None,
            stdin: None,
            stdout: None,
            exe_path: runtime_exe_path(data_dir),
        }
    }

    pub fn runtime_exists(&self) -> bool {
        self.exe_path.exists()
    }

    pub async fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        if !self.exe_path.exists() {
            return Err(format!(
                "VITS runtime not found at {}",
                self.exe_path.display()
            ));
        }
        logger::info(&format!("[VITS] starting runtime: {}", self.exe_path.display()));
        let mut cmd = Command::new(&self.exe_path);
        cmd.current_dir(self.exe_path.parent().unwrap_or_else(|| Path::new(".")))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd.env("PYTHONIOENCODING", "utf-8");
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW：避免弹出控制台黑窗口
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn VITS runtime: {}", e))?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(BufReader::new);
        self.child = Some(child);
        if let Err(e) = self.wait_ready().await {
            self.stop();
            return Err(e);
        }
        logger::info("[VITS] runtime ready");
        Ok(())
    }

    async fn wait_ready(&mut self) -> Result<(), String> {
        let mut line = String::new();
        if let Some(ref mut stdout) = self.stdout {
            let read = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                stdout.read_line(&mut line),
            )
            .await
            .map_err(|_| "VITS runtime ready timeout".to_string())?
            .map_err(|e| e.to_string())?;
            if read == 0 {
                return Err("VITS runtime exited before ready".into());
            }
            let ping: VitsPingResponse = serde_json::from_str(line.trim())
                .map_err(|e| format!("Invalid ready signal: {}", e))?;
            if ping.ready {
                return Ok(());
            }
        }
        Err("VITS runtime not ready".into())
    }

    pub async fn generate(&mut self, req: &VitsRequest) -> Result<VitsResponse, String> {
        if self.child.is_none() {
            self.start().await?;
        }
        let mut stdin = self.stdin.take().ok_or("VITS runtime stdin missing")?;
        let mut stdout = self.stdout.take().ok_or("VITS runtime stdout missing")?;

        let result = Self::send_and_recv(&mut stdin, &mut stdout, req).await;

        match result {
            Ok(resp) => {
                self.stdin = Some(stdin);
                self.stdout = Some(stdout);
                Ok(resp)
            }
            Err(e) => {
                // 通信失败说明子进程状态不可靠，直接终止，下次调用时重启
                logger::error(&format!("[VITS] communication failed, restarting runtime: {}", e));
                drop(stdin);
                drop(stdout);
                self.stop();
                Err(e)
            }
        }
    }

    async fn send_and_recv(
        stdin: &mut ChildStdin,
        stdout: &mut BufReader<ChildStdout>,
        req: &VitsRequest,
    ) -> Result<VitsResponse, String> {
        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        stdin
            .write_all(json.as_bytes())
            .await
            .map_err(|e| format!("VITS write failed: {}", e))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("VITS write failed: {}", e))?;
        stdin.flush().await.map_err(|e| e.to_string())?;

        let mut line = String::new();
        // 语音合成可能耗时较长，给予 10 分钟超时
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(600),
            stdout.read_line(&mut line),
        )
        .await
        .map_err(|_| "VITS generation timeout".to_string())?
        .map_err(|e| format!("VITS read failed: {}", e))?;
        if read == 0 {
            return Err("VITS runtime exited unexpectedly".into());
        }
        serde_json::from_str(line.trim()).map_err(|e| format!("Invalid VITS response: {}", e))
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        self.stdin = None;
        self.stdout = None;
    }
}

impl Drop for VitsRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
