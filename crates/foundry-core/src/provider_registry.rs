pub use provider_pygo::{
    JavaStackProvider, PythonStackProvider, RustStackProvider, StackRegistry, TechStackProvider,
    WebHtmlStackProvider,
};

use foundry_proto::TargetStack;
use std::sync::Arc;

/// 全局技术栈提供者调度管理器
pub struct ProviderManager {
    active_stack: TargetStack,
}

impl ProviderManager {
    pub fn new(active_stack: TargetStack) -> Self {
        Self { active_stack }
    }

    pub fn get_active_stack(&self) -> TargetStack {
        self.active_stack
    }

    pub fn set_active_stack(&mut self, stack: TargetStack) {
        self.active_stack = stack;
    }

    pub fn get_active_provider(&self) -> Arc<dyn TechStackProvider> {
        StackRegistry::get_provider(self.active_stack)
    }
}
