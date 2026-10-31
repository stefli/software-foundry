use crate::pi::memory::PiProjectMemory;
use tracing::info;

/// Policy 策略仲裁输出的下一步行动动作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextPolicyAction {
    /// 前台对话轨：诉求模糊，需要向用户提问澄清 (Listening / Probing)
    AskClarification { prompt_to_user: String },

    /// 前台对话轨：诉求已清晰，激活需求架构师 Agent 生成业务规约与数据库表
    DecomposeRequirements,

    /// 前台对话轨：等待用户选定视觉色系 (Design Tokens)
    PromptChooseDesignToken,

    /// 后台执行轨：共识已达成，激活 Coder Skill 编写/重构源码
    TriggerCodeGeneration,

    /// 后台执行轨：源码编译失败，触发自愈反思循环 (Self-Healing)
    TriggerSelfHealing { error_feedback: String, attempt: u32 },

    /// 策略安全门禁 (HITL)：检测到歧义或破坏性操作，暂停执行流等待人类审批
    AwaitingHumanApproval { reason: String },

    /// 交付状态：沙箱验证全绿色通过，生成成品
    DeliverArtifact,
}

/// Pi-Harness 策略仲裁引擎 (Policy Engine)
pub struct PiPolicyEngine {
    pub max_self_healing_attempts: u32,
}

impl Default for PiPolicyEngine {
    fn default() -> Self {
        Self {
            max_self_healing_attempts: 4,
        }
    }
}

impl PiPolicyEngine {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_self_healing_attempts: max_attempts,
        }
    }

    /// 根据三态记忆上下文，自主裁决下一步最佳策略 (Policy Evaluation)
    pub fn evaluate(&self, memory: &PiProjectMemory) -> NextPolicyAction {
        // 1. 若连原始诉求都没有，继续倾听
        if memory.intent.raw_idea.trim().is_empty() {
            return NextPolicyAction::AskClarification {
                prompt_to_user: "请告诉我您想要制造一个什么样的软件？".to_string(),
            };
        }

        // 2. 如果诉求太简短模糊（少于 5 个字），主动追问
        if memory.intent.raw_idea.chars().count() < 5 && memory.intent.spec.is_none() {
            return NextPolicyAction::AskClarification {
                prompt_to_user: "您的想法很有趣！能否具体说说主要需要哪些核心功能？".to_string(),
            };
        }

        // 3. 如果有诉求但尚未做系统拆解，裁决执行需求架构师分析
        if memory.intent.spec.is_none() {
            info!("📋 [Policy] 诉求明确，裁决激活需求架构师 Agent 进行结构化拆解");
            return NextPolicyAction::DecomposeRequirements;
        }

        // 4. 检查沙箱执行状态：如果上一轮报错了
        if let Some(exit_code) = memory.runtime.last_exit_code {
            if exit_code != 0 {
                // 如果已超过最大自愈轮次，触发人工审批门禁 (HITL)，绝不死循环
                if memory.runtime.iteration_count >= self.max_self_healing_attempts {
                    info!("🛑 [Policy Guardrail] 自愈重试达到阈值，触发人工干预门禁");
                    return NextPolicyAction::AwaitingHumanApproval {
                        reason: format!(
                            "沙箱连续 {} 次自愈未通过，最近错误: {}",
                            memory.runtime.iteration_count,
                            memory.runtime.last_traceback.as_deref().unwrap_or("未知")
                        ),
                    };
                }

                // 否则激活自愈策略
                info!("🛠️ [Policy] 沙箱有报错，裁决触发自愈反思重试");
                return NextPolicyAction::TriggerSelfHealing {
                    error_feedback: memory.runtime.last_traceback.clone().unwrap_or_default(),
                    attempt: memory.runtime.iteration_count,
                };
            }
        }

        // 5. 如果源码存在且沙箱通过，裁决交付
        if memory.runtime.current_code.is_some() && memory.runtime.is_clean() {
            info!("🏆 [Policy] 沙箱冒烟探活 100% 通过，裁决交付成品");
            return NextPolicyAction::DeliverArtifact;
        }

        // 6. 默认进入编码阶段
        NextPolicyAction::TriggerCodeGeneration
    }
}
