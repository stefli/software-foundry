use anyhow::{Context, Result};
use foundry_plugin_api::ResourceConnectorPlugin;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 微内核插件注册表与热插拔调度中心
pub struct PluginRegistry {
    connectors: RwLock<HashMap<String, Arc<RwLock<dyn ResourceConnectorPlugin>>>>,
    active_connector_id: RwLock<Option<String>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            connectors: RwLock::new(HashMap::new()),
            active_connector_id: RwLock::new(None),
        }
    }

    /// 注册/插入一个资源连接器插件
    pub async fn register_connector(&self, connector: Arc<RwLock<dyn ResourceConnectorPlugin>>) {
        let id = {
            let guard = connector.read().await;
            guard.id().to_string()
        };
        let mut map = self.connectors.write().await;
        map.insert(id.clone(), connector);

        // 如果当前没有激活的连接器，自动将首个作为默认
        let mut active = self.active_connector_id.write().await;
        if active.is_none() {
            *active = Some(id);
        }
    }

    /// 切换当前激活的资源连接器插件
    pub async fn switch_active_connector(&self, connector_id: &str) -> Result<()> {
        let map = self.connectors.read().await;
        if map.contains_key(connector_id) {
            let mut active = self.active_connector_id.write().await;
            *active = Some(connector_id.to_string());
            Ok(())
        } else {
            anyhow::bail!("未找到指定的资源连接器插件: {}", connector_id);
        }
    }

    /// 获取当前生效的资源连接器插件
    pub async fn get_active_connector(&self) -> Result<Arc<RwLock<dyn ResourceConnectorPlugin>>> {
        let active_opt = self.active_connector_id.read().await.clone();
        let active_id = active_opt.context("当前没有激活的资源连接器插件")?;

        let map = self.connectors.read().await;
        let connector = map
            .get(&active_id)
            .cloned()
            .context(format!("未找到激活的插件: {}", active_id))?;
        Ok(connector)
    }

    /// 列出所有已挂载的资源插件及其实时连接状态
    pub async fn list_connectors(&self) -> Vec<(String, String, bool)> {
        let map = self.connectors.read().await;
        let mut list = Vec::new();
        for (id, plugin) in map.iter() {
            let guard = plugin.read().await;
            let name = guard.display_name().to_string();
            let connected = guard.is_connected().await;
            list.push((id.clone(), name, connected));
        }
        list
    }
}
