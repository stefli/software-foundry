use anyhow::Result;
use async_trait::async_trait;
use foundry_proto::AppToolDefinition;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

/// 插件类型分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// 大模型推理网关插件 (Magpie, Gemini, DeepSeek)
    LlmGateway,
    /// 专职智能体技能插件 (需求分析师, 设计系统, 编码工程师, 质检员)
    Skill,
    /// 外部基础设施连接器插件 (GitHub, SQLite 沙箱, Docker, Cloudflare)
    Connector,
}

/// 全局一切皆是插件的标准契约 (Unified Plugin Contract)
#[async_trait]
pub trait FoundryPlugin: Send + Sync {
    /// 唯一标识符 (例如: "magpie_gateway", "requirements_skill", "github_connector")
    fn id(&self) -> &'static str;

    /// 显示名称
    fn name(&self) -> &'static str;

    /// 插件分类
    fn kind(&self) -> PluginKind;

    /// 描述
    fn description(&self) -> &'static str;

    /// 插件初始化加载钩子
    async fn on_load(&self) -> Result<()> {
        Ok(())
    }

    /// 暴露给母机与 Agent 可调用的工具集契约 (MCP / Actions)
    fn get_tools(&self) -> Vec<AppToolDefinition> {
        vec![]
    }

    /// 统一调用该插件暴露的具体工具能力
    async fn call_tool(&self, _tool_name: &str, _params: Value) -> Result<Value> {
        anyhow::bail!("插件 [{}] 未实现工具调用", self.id())
    }
}

/// Pi-Harness 插件注册与能力总线 (Plugin Registry)
#[derive(Default)]
pub struct PiPluginRegistry {
    plugins: HashMap<&'static str, Arc<dyn FoundryPlugin>>,
}

impl PiPluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// 注册插件
    pub fn register<P: FoundryPlugin + 'static>(&mut self, plugin: P) {
        let id = plugin.id();
        info!("🔌 [Pi-Harness] 成功注册插件: [{}] ({:?}) - {}", id, plugin.kind(), plugin.name());
        self.plugins.insert(id, Arc::new(plugin));
    }

    /// 获取指定插件
    pub fn get(&self, id: &str) -> Option<Arc<dyn FoundryPlugin>> {
        self.plugins.get(id).cloned()
    }

    /// 按类型检索所有可用插件
    pub fn list_by_kind(&self, kind: PluginKind) -> Vec<Arc<dyn FoundryPlugin>> {
        self.plugins
            .values()
            .filter(|p| p.kind() == kind)
            .cloned()
            .collect()
    }

    /// 汇集所有已安装插件暴露给 Agent 的完整工具箱
    pub fn get_all_tools(&self) -> Vec<AppToolDefinition> {
        let mut tools = Vec::new();
        for plugin in self.plugins.values() {
            tools.extend(plugin.get_tools());
        }
        tools
    }

    /// 统一执行工具调用（由总线自动路由到对应插件）
    pub async fn dispatch_tool_call(&self, tool_name: &str, params: Value) -> Result<Value> {
        for plugin in self.plugins.values() {
            for tool in plugin.get_tools() {
                if tool.name == tool_name {
                    return plugin.call_tool(tool_name, params).await;
                }
            }
        }
        anyhow::bail!("未找到提供工具 [{}] 的活跃插件", tool_name)
    }
}
