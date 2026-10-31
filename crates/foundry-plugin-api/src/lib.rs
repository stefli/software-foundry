use anyhow::Result;
use async_trait::async_trait;
use foundry_proto::Issue;
use std::sync::Arc;

/// 插件类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// 资源连接器 (代码托管平台、Issue 来源、CI/CD 系统)
    ResourceConnector,
    /// 生命周期 Bot (需求分析、架构设计、代码生成)
    LifecycleBot,
    /// 技术栈提供者 (Python, Go, .NET, Vue 等)
    StackProvider,
}

/// 资源连接器插件能力规范 (所有外部资源连接必须以此契约接入，实现即插即用)
#[async_trait]
pub trait ResourceConnectorPlugin: Send + Sync {
    /// 插件唯一标识符 (例如 "connector-github", "connector-gitlab")
    fn id(&self) -> &'static str;

    /// 插件展示名称
    fn display_name(&self) -> &'static str;

    /// 插件描述
    fn description(&self) -> &'static str;

    /// 是否已经连接并授权有效
    async fn is_connected(&self) -> bool;

    /// 触发交互式授权流程 (如弹出浏览器、输入 Token 等)
    async fn trigger_auth_flow(&mut self) -> Result<()>;

    /// 断开当前连接 / 清理配置
    async fn disconnect(&mut self) -> Result<()>;

    /// 拉取当前资源所关联的待处理需求列表
    async fn fetch_issues(&self) -> Result<Vec<Issue>>;

    /// 回写需求/Issue 进度评论
    async fn post_comment(&self, issue_id: &str, comment: &str) -> Result<()>;
}

/// 工厂通用插件基础特征
pub trait FoundryPlugin: Send + Sync {
    fn kind(&self) -> PluginKind;
    fn as_resource_connector(&self) -> Option<Arc<dyn ResourceConnectorPlugin>> {
        None
    }
}
