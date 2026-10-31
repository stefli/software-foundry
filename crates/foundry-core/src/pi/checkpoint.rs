use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// 单个沙箱时间线快照
#[derive(Debug, Clone)]
pub struct SandboxSnapshot {
    pub checkpoint_id: u32,
    pub note: String,
    pub files: HashMap<String, String>, // 文件相对路径 -> 完整源码内容
    pub timestamp: String,
}

/// 沙箱时光旅行回滚引擎 (Time-Travel Checkpoint Engine)
pub struct CheckpointEngine {
    sandbox_root: PathBuf,
    snapshots: Vec<SandboxSnapshot>,
    next_id: u32,
}

impl CheckpointEngine {
    pub fn new<P: AsRef<Path>>(sandbox_root: P) -> Self {
        Self {
            sandbox_root: sandbox_root.as_ref().to_path_buf(),
            snapshots: Vec::new(),
            next_id: 1,
        }
    }

    /// 在执行潜在修改前，为沙箱文件建立轻量级内存树快照
    pub fn capture(&mut self, note: impl Into<String>) -> Result<u32> {
        let mut files_map = HashMap::new();

        if self.sandbox_root.exists() {
            for entry in std::fs::read_dir(&self.sandbox_root)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        // 只备份源码与配置，不备份二进制或大缓存
                        if file_name.ends_with(".py") || file_name.ends_with(".json") || file_name.ends_with(".sql") {
                            let content = std::fs::read_to_string(&path)?;
                            files_map.insert(file_name.to_string(), content);
                        }
                    }
                }
            }
        }

        let cid = self.next_id;
        self.next_id += 1;

        let snapshot = SandboxSnapshot {
            checkpoint_id: cid,
            note: note.into(),
            files: files_map,
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        };

        info!("📸 [Checkpoint] 创建沙箱状态快照 #{} ({})", cid, snapshot.note);
        self.snapshots.push(snapshot);
        Ok(cid)
    }

    /// 一键时光倒流：将沙箱恢复至指定快照版本
    pub fn rollback_to(&self, checkpoint_id: u32) -> Result<()> {
        let snap = self
            .snapshots
            .iter()
            .find(|s| s.checkpoint_id == checkpoint_id)
            .with_context(|| format!("未找到指定的快照记录 #{}", checkpoint_id))?;

        info!("⏪ [Time-Travel Rollback] 正在将沙箱代码回滚至快照 #{}: {}", snap.checkpoint_id, snap.note);

        for (file_name, content) in &snap.files {
            let target = self.sandbox_root.join(file_name);
            std::fs::write(&target, content)
                .with_context(|| format!("回滚文件失败: {:?}", target))?;
        }

        info!("✔ 沙箱时光倒流完成，已安全复原至稳定版本！");
        Ok(())
    }

    /// 回滚到最近一次稳定快照
    pub fn rollback_last(&self) -> Result<()> {
        if let Some(last) = self.snapshots.last() {
            self.rollback_to(last.checkpoint_id)
        } else {
            anyhow::bail!("尚无可用快照可供回滚")
        }
    }
}
