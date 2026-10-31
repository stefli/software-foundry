use anyhow::{Context, Result};
use foundry_proto::{SandboxExecutionResult, SandboxExecutionSpec};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, info};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// 沙箱工作空间上下文
pub struct SandboxWorkspace {
    pub root_dir: PathBuf,
    pub sqlite_db_path: PathBuf,
    pub is_ephemeral: bool,
}

impl SandboxWorkspace {
    /// 创建一个新的沙箱工作空间，自动配置独立的 SQLite 数据库路径
    pub fn new<P: AsRef<Path>>(target_dir: P) -> Result<Self> {
        let root_dir = target_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&root_dir)
            .with_context(|| format!("Failed to create sandbox directory: {:?}", root_dir))?;

        let sqlite_db_path = root_dir.join("foundry_test_runtime.db");

        Ok(Self {
            root_dir,
            sqlite_db_path,
            is_ephemeral: false,
        })
    }

    /// 获取注入了 SQLite 隔离配置的标准环境变量集
    pub fn get_injected_env(&self) -> HashMap<String, String> {
        let mut envs = HashMap::new();
        let db_str = self.sqlite_db_path.to_string_lossy().to_string();

        envs.insert("FOUNDRY_SANDBOX".to_string(), "1".to_string());
        envs.insert("SQLITE_DATABASE_PATH".to_string(), db_str.clone());
        envs.insert(
            "DATABASE_URL".to_string(),
            format!("sqlite://{}", db_str),
        );
        envs
    }

    /// 执行指定的命令并在沙箱中捕获结果
    pub async fn execute(&self, spec: &SandboxExecutionSpec) -> Result<SandboxExecutionResult> {
        info!(
            "Running in sandbox: {} {} (at {:?})",
            spec.command,
            spec.args.join(" "),
            spec.working_dir
        );

        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args);

        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW 消除 Windows CMD 弹出窗口闪烁

        // 设置工作目录
        let working_path = if spec.working_dir.is_empty() {
            &self.root_dir
        } else {
            Path::new(&spec.working_dir)
        };
        cmd.current_dir(working_path);

        // 注入默认沙箱环境变量与外部传入的环境变量
        for (k, v) in self.get_injected_env() {
            cmd.env(k, v);
        }
        for (k, v) in &spec.env_vars {
            cmd.env(k, v);
        }

        let start_time = Instant::now();

        // 异步等待执行，超时管理
        let timeout_duration = std::time::Duration::from_secs(if spec.timeout_seconds == 0 {
            120
        } else {
            spec.timeout_seconds
        });

        let output_future = cmd.output();
        let output = tokio::time::timeout(timeout_duration, output_future)
            .await
            .context("Sandbox execution timed out")?
            .context("Failed to execute process in sandbox")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        let passed = output.status.success();

        debug!(
            "Sandbox run completed with code {} in {}ms",
            exit_code, duration_ms
        );

        Ok(SandboxExecutionResult {
            exit_code,
            stdout,
            stderr,
            duration_ms,
            passed,
        })
    }
}
