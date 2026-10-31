use foundry_proto::ProjectSpecDecomposition;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 1. 工作记忆 (Working Memory): 当前轮次的高精度即时会话上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMessage {
    pub is_user: bool,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemory {
    max_turns: usize,
    messages: VecDeque<WorkingMessage>,
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self {
            max_turns: 5, // 仅保留最近 5 轮，避免无谓膨胀
            messages: VecDeque::new(),
        }
    }
}

impl WorkingMemory {
    pub fn new(max_turns: usize) -> Self {
        Self {
            max_turns,
            messages: VecDeque::new(),
        }
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.push(true, text);
    }

    pub fn push_agent(&mut self, text: impl Into<String>) {
        self.push(false, text);
    }

    fn push(&mut self, is_user: bool, content: impl Into<String>) {
        if self.messages.len() >= self.max_turns * 2 {
            self.messages.pop_front();
        }
        self.messages.push_back(WorkingMessage {
            is_user,
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }

    pub fn get_recent_history(&self) -> Vec<WorkingMessage> {
        self.messages.iter().cloned().collect()
    }
}

/// 2. 意图快照 (Intent Snapshot): 动态蒸馏出的结构化事实共识 (几百 Token 恒定不爆)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntentSnapshot {
    pub raw_idea: String,
    pub spec: Option<ProjectSpecDecomposition>,
    pub confirmed_at: Option<String>,
}

impl IntentSnapshot {
    pub fn update_spec(&mut self, idea: &str, spec: ProjectSpecDecomposition) {
        self.raw_idea = idea.to_string();
        self.spec = Some(spec);
        self.confirmed_at = Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    }

    pub fn is_confirmed(&self) -> bool {
        self.spec.is_some()
    }
}

/// 3. 运行时状态记忆 (Runtime State Memory): 真实沙箱上下文与自愈线索
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeStateMemory {
    pub current_code: Option<String>,
    pub last_exit_code: Option<i32>,
    pub last_traceback: Option<String>,
    pub db_initialized: bool,
    pub iteration_count: u32,
}

impl RuntimeStateMemory {
    pub fn record_sandbox_run(&mut self, exit_code: i32, output_err: Option<String>) {
        self.last_exit_code = Some(exit_code);
        self.last_traceback = output_err;
        self.iteration_count += 1;
    }

    pub fn is_clean(&self) -> bool {
        self.last_exit_code == Some(0)
    }
}

/// 统一聚合的 Pi 项目三态记忆总线 (Project Memory Bus)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiProjectMemory {
    pub working: WorkingMemory,
    pub intent: IntentSnapshot,
    pub runtime: RuntimeStateMemory,
}

impl PiProjectMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// 为 Agent 组装高度精炼的 Prompt 上下文 (杜绝无脑拼接历史)
    pub fn assemble_prompt_context(&self) -> String {
        let mut ctx = String::new();

        // 注入当前确认的意图核心事实
        if let Some(ref spec) = self.intent.spec {
            ctx.push_str(&format!(
                "【系统定位】: {}\n【核心业务模块】:\n",
                spec.positioning
            ));
            for m in &spec.modules {
                ctx.push_str(&format!("- [{}] {}: {}\n", m.priority, m.name, m.desc));
            }
        } else if !self.intent.raw_idea.is_empty() {
            ctx.push_str(&format!("【原始诉求】: {}\n", self.intent.raw_idea));
        }

        // 若上一轮沙箱执行有报错，注入自愈线索
        if let Some(ref err) = self.runtime.last_traceback {
            ctx.push_str(&format!("\n【上一轮沙箱报错 (自愈目标)】:\n{}\n", err));
        }

        // 仅注入最近几轮会话
        let recent = self.working.get_recent_history();
        if !recent.is_empty() {
            ctx.push_str("\n【最近沟通上下文】:\n");
            for msg in recent {
                let role = if msg.is_user { "用户" } else { "AI顾问" };
                ctx.push_str(&format!("{}: {}\n", role, msg.content));
            }
        }

        ctx
    }
}
