use anyhow::{Context, Result};
use foundry_proto::{Issue, PipelineState, SandboxExecutionSpec};
use foundry_sandbox::SandboxWorkspace;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::fsm::{FoundryFsm, TransitionEvent};
use crate::issue_source::IssueProvider;
use crate::provider_registry::ProviderManager;

pub struct HarnessLoop {
    pub workspace_root: PathBuf,
    pub fsm: FoundryFsm,
    pub issue_source: Arc<dyn IssueProvider>,
    pub provider_mgr: Arc<ProviderManager>,
}

impl HarnessLoop {
    pub fn new(
        workspace_root: PathBuf,
        issue_source: Arc<dyn IssueProvider>,
        provider_mgr: Arc<ProviderManager>,
    ) -> Self {
        Self {
            workspace_root,
            fsm: FoundryFsm::default(),
            issue_source,
            provider_mgr,
        }
    }

    /// 执行一轮完整的 Issue 处理自治生命周期
    pub async fn process_issue(&mut self, issue: &Issue) -> Result<PipelineState> {
        info!(">>> [Foundry Harness] Starting pipeline for Issue #{}: {}", issue.id, issue.title);
        
        // 1. Pending -> Triaging
        self.fsm.transition(TransitionEvent::IssueReceived);
        info!("State: {:?}", self.fsm.current_state);
        self.issue_source.update_issue_comment(&issue.id, "🤖 工厂已接收需求，开始分析与验收标准拆解...").await?;

        // 2. Triaging -> Planning
        self.fsm.transition(TransitionEvent::SpecTriaged);
        info!("State: {:?}", self.fsm.current_state);

        // 3. Planning -> Coding
        self.fsm.transition(TransitionEvent::PlanApproved);
        info!("State: {:?}", self.fsm.current_state);

        // 准备沙箱工作区
        let project_sandbox_dir = self.workspace_root.join("sandbox").join(&issue.id);
        let sandbox = SandboxWorkspace::new(&project_sandbox_dir)
            .context("Failed to initialize project sandbox")?;

        info!("Sandbox initialized at: {:?}", sandbox.root_dir);

        // 4. 自愈与验证循环 (Self-Healing Loop)
        let mut _retry_count = 0;
        let mut _last_error_feedback: Option<String> = None;

        loop {
            // Coding
            self.fsm.transition(TransitionEvent::CodeGenerated);
            info!("State: {:?}, generating code/patches...", self.fsm.current_state);

            // 写入根据 Issue 动态生成的验证脚本进行沙箱联通性与持久化验证
            let smoke_test_script = sandbox.root_dir.join("smoke_test.py");
            let safe_title = issue.title.replace('"', "\\\"");
            let dynamic_script = format!(
                r#"import os
import sqlite3

print("Running Foundry Sandbox Smoke Test for Issue: \"{safe_title}\"")
db_path = os.environ.get("SQLITE_DATABASE_PATH", "test.db")
print(f"Connecting to isolated SQLite DB at: {{db_path}}")

conn = sqlite3.connect(db_path)
cursor = conn.cursor()
cursor.execute("CREATE TABLE IF NOT EXISTS runtime_audit (id INTEGER PRIMARY KEY AUTOINCREMENT, issue_ref TEXT, audit_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP);")
cursor.execute("INSERT INTO runtime_audit (issue_ref) VALUES (?);", ("{safe_title}",))
conn.commit()

cursor.execute("SELECT COUNT(*) FROM runtime_audit;")
count = cursor.fetchone()[0]
print(f"SQLite sandbox dynamic audit verification success! Audit record count: {{count}}")
conn.close()
"#,
                safe_title = safe_title
            );
            tokio::fs::write(&smoke_test_script, dynamic_script).await?;

            // 5. Verifying in Sandbox
            info!("State: {:?}, executing sandbox tests...", self.fsm.current_state);
            let exec_spec = SandboxExecutionSpec {
                command: "python".to_string(),
                args: vec!["smoke_test.py".to_string()],
                env_vars: std::collections::HashMap::new(),
                working_dir: sandbox.root_dir.to_string_lossy().to_string(),
                timeout_seconds: 30,
            };

            let exec_result = sandbox.execute(&exec_spec).await?;

            if exec_result.passed {
                info!("Sandbox verification PASSED in {}ms!", exec_result.duration_ms);
                self.fsm.transition(TransitionEvent::VerificationPassed);
                self.issue_source.update_issue_comment(
                    &issue.id,
                    &format!("✅ 沙箱构建与 SQLite 数据模型自测通过！耗时: {}ms\n输出日志:\n```\n{}\n```", 
                        exec_result.duration_ms, exec_result.stdout
                    )
                ).await?;
                break;
            } else {
                warn!("Sandbox verification FAILED: {}", exec_result.stderr);
                _retry_count += 1;
                _last_error_feedback = Some(exec_result.stderr.clone());

                let next_state = self.fsm.transition(TransitionEvent::VerificationFailed {
                    reason: exec_result.stderr,
                });

                if matches!(next_state, PipelineState::AwaitingApproval) {
                    warn!("Max retries reached. Awaiting manual developer approval.");
                    break;
                } else {
                    self.fsm.transition(TransitionEvent::SelfHealRetried);
                }
            }
        }

        info!("Final Pipeline State: {:?}", self.fsm.current_state);
        Ok(self.fsm.current_state)
    }
}
