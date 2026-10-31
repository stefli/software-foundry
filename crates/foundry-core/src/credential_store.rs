use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

/// 存储在操作系统用户目录下的私密凭据（不随项目 Git 提交）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecureCredentials {
    pub gemini_api_key: Option<String>,
    pub magpie_api_key: Option<String>,
    pub magpie_base_url: Option<String>,
    pub github_token: Option<String>,
}

pub struct CredentialStore {
    secrets_path: PathBuf,
}

impl CredentialStore {
    pub fn new() -> Self {
        let user_home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        
        let foundry_dir = PathBuf::from(user_home).join(".foundry");
        let secrets_path = foundry_dir.join("secrets.json");

        Self { secrets_path }
    }

    /// 读取已保存的安全凭据（优先返回环境变量中的 Key）
    pub fn load_gemini_key(&self) -> Option<String> {
        // 1. 优先读取系统环境变量 (最安全方式)
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Ok(key) = std::env::var("GOOGLE_API_KEY") {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // 2. 从本地隔离的安全配置文件读取
        if self.secrets_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&self.secrets_path) {
                if let Ok(creds) = serde_json::from_str::<SecureCredentials>(&data) {
                    if let Some(ref k) = creds.gemini_api_key {
                        let trimmed = k.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    /// 读取已保存的 Magpie 网关凭据
    pub fn load_magpie_credentials(&self) -> (Option<String>, Option<String>) {
        if let Ok(key) = std::env::var("MAGPIE_API_KEY") {
            let base = std::env::var("MAGPIE_BASE_URL").ok();
            return (Some(key), base);
        }

        if self.secrets_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&self.secrets_path) {
                if let Ok(creds) = serde_json::from_str::<SecureCredentials>(&data) {
                    return (creds.magpie_api_key, creds.magpie_base_url);
                }
            }
        }

        (None, None)
    }

    /// 保存 Magpie 凭据
    pub fn save_magpie_credentials(&self, key: &str, base_url: &str) -> Result<()> {
        let mut creds = if self.secrets_path.exists() {
            let data = std::fs::read_to_string(&self.secrets_path).unwrap_or_default();
            serde_json::from_str::<SecureCredentials>(&data).unwrap_or_default()
        } else {
            SecureCredentials::default()
        };

        creds.magpie_api_key = Some(key.trim().to_string());
        creds.magpie_base_url = Some(base_url.trim().to_string());

        if let Some(parent) = self.secrets_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serde_json::to_string_pretty(&creds)?;
        std::fs::write(&self.secrets_path, serialized)?;
        info!("Magpie 网关凭据已安全持久化保存于: {:?}", self.secrets_path);
        Ok(())
    }

    /// 安全持久化保存 Gemini Key
    pub fn save_gemini_key(&self, key: &str) -> Result<()> {
        let clean_key = key.trim();
        let mut creds = if self.secrets_path.exists() {
            let data = std::fs::read_to_string(&self.secrets_path).unwrap_or_default();
            serde_json::from_str::<SecureCredentials>(&data).unwrap_or_default()
        } else {
            SecureCredentials::default()
        };

        creds.gemini_api_key = Some(clean_key.to_string());

        if let Some(parent) = self.secrets_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建私密存储目录: {:?}", parent))?;
        }

        let serialized = serde_json::to_string_pretty(&creds)?;
        std::fs::write(&self.secrets_path, serialized)?;
        info!("Gemini API Key 已安全加密存储在用户目录: {:?}", self.secrets_path);
        Ok(())
    }

    /// 保存 GitHub Token
    pub fn save_github_token(&self, token: &str) -> Result<()> {
        let clean_token = token.trim();
        let mut creds = if self.secrets_path.exists() {
            let data = std::fs::read_to_string(&self.secrets_path).unwrap_or_default();
            serde_json::from_str::<SecureCredentials>(&data).unwrap_or_default()
        } else {
            SecureCredentials::default()
        };

        creds.github_token = Some(clean_token.to_string());

        if let Some(parent) = self.secrets_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建私密存储目录: {:?}", parent))?;
        }

        let serialized = serde_json::to_string_pretty(&creds)?;
        std::fs::write(&self.secrets_path, serialized)?;
        info!("GitHub Token 已安全加密存储在用户目录: {:?}", self.secrets_path);
        Ok(())
    }

    /// 读取已保存的 GitHub Token
    pub fn load_github_token(&self) -> Option<String> {
        if let Ok(tok) = std::env::var("GITHUB_TOKEN") {
            let trimmed = tok.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if self.secrets_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&self.secrets_path) {
                if let Ok(creds) = serde_json::from_str::<SecureCredentials>(&data) {
                    if let Some(ref t) = creds.github_token {
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// 遮罩辅助函数 (如 AIzaSyD...X79)
    pub fn mask_key(key: &str) -> String {
        let len = key.len();
        if len <= 8 {
            "••••••••".to_string()
        } else {
            format!("{}••••••••{}", &key[..6], &key[len - 4..])
        }
    }
}
