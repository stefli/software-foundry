use foundry_proto::PipelineState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionEvent {
    IssueReceived,
    SpecTriaged,
    PlanApproved,
    CodeGenerated,
    VerificationPassed,
    VerificationFailed { reason: String },
    SelfHealRetried,
    MaxRetriesExceeded,
    ManualApproved,
}

#[derive(Debug, Clone)]
pub struct FoundryFsm {
    pub current_state: PipelineState,
    pub attempts: u32,
    pub max_attempts: u32,
}

impl Default for FoundryFsm {
    fn default() -> Self {
        Self {
            current_state: PipelineState::Pending,
            attempts: 0,
            max_attempts: 5,
        }
    }
}

impl FoundryFsm {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            current_state: PipelineState::Pending,
            attempts: 0,
            max_attempts,
        }
    }

    /// 执行状态机转移
    pub fn transition(&mut self, event: TransitionEvent) -> PipelineState {
        self.current_state = match (self.current_state, event) {
            (PipelineState::Pending, TransitionEvent::IssueReceived) => PipelineState::Triaging,
            (PipelineState::Triaging, TransitionEvent::SpecTriaged) => PipelineState::Planning,
            (PipelineState::Planning, TransitionEvent::PlanApproved) => PipelineState::Coding,
            (PipelineState::Coding, TransitionEvent::CodeGenerated) => PipelineState::Verifying,
            (PipelineState::Verifying, TransitionEvent::VerificationPassed) => {
                PipelineState::Completed
            }
            (PipelineState::Verifying, TransitionEvent::VerificationFailed { .. }) => {
                self.attempts += 1;
                if self.attempts >= self.max_attempts {
                    PipelineState::AwaitingApproval
                } else {
                    PipelineState::SelfHealing {
                        attempts: self.attempts,
                        max_attempts: self.max_attempts,
                    }
                }
            }
            (PipelineState::SelfHealing { .. }, TransitionEvent::SelfHealRetried) => {
                PipelineState::Coding
            }
            (PipelineState::SelfHealing { .. }, TransitionEvent::MaxRetriesExceeded) => {
                PipelineState::AwaitingApproval
            }
            (PipelineState::AwaitingApproval, TransitionEvent::ManualApproved) => {
                self.attempts = 0;
                PipelineState::Coding
            }
            (state, _) => state, // 无效转换保持现状
        };
        self.current_state
    }
}
