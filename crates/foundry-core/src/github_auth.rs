use anyhow::{Context, Result};
use foundry_proto::{GitHubConfig, GitHubDeviceCodeResponse};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

// 预设公共应用 Client ID（或用户可在配置中自定义覆盖）
pub const DEFAULT_GITHUB_CLIENT_ID: &str = "017c3d53e7a363b169d8"; // 标准公共只读/Issue CLI Client ID

pub struct GitHubAuthManager {
    config_file_path: PathBuf,
    client: reqwest::Client,
    pub config: GitHubConfig,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUserResponse {
    login: String,
}

impl GitHubAuthManager {
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        let config_file_path = config_dir.as_ref().join("github_auth.json");
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.v3+json, application/json"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Foundry-Software-Factory/0.1"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        let mut mgr = Self {
            config_file_path,
            client,
            config: GitHubConfig::default(),
        };

        let _ = mgr.load_config();
        mgr
    }

    /// 从本地加载已有的授权配置
    pub fn load_config(&mut self) -> Result<()> {
        if self.config_file_path.exists() {
            let data = std::fs::read_to_string(&self.config_file_path)?;
            self.config = serde_json::from_str(&data)?;
        }
        Ok(())
    }

    /// 持久化保存授权配置
    pub fn save_config(&self) -> Result<()> {
        if let Some(parent) = self.config_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.config_file_path, data)?;
        Ok(())
    }

    /// 检查是否已经授权连接
    pub fn is_authenticated(&self) -> bool {
        self.config.access_token.is_some()
    }

    /// 发起 Device Code Flow 请求，自动弹出浏览器授权页面
    pub async fn start_device_flow(
        &self,
        custom_client_id: Option<String>,
    ) -> Result<GitHubDeviceCodeResponse> {
        let client_id = custom_client_id
            .or_else(|| self.config.client_id.clone())
            .unwrap_or_else(|| DEFAULT_GITHUB_CLIENT_ID.to_string());

        let res = self
            .client
            .post("https://github.com/login/device/code")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": client_id,
                "scope": "repo,read:user"
            }))
            .send()
            .await
            .context("Failed to request GitHub device code")?;

        let status = res.status();
        let body_text = res.text().await.context("Failed to read response body")?;

        if !status.is_success() {
            anyhow::bail!("GitHub device code API returned {}: {}", status, body_text);
        }

        let device_code_resp: GitHubDeviceCodeResponse = serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse device code response: {}", body_text))?;

        info!(
            "🔑 [GitHub Auth] 授权用户码: {} (请在浏览器中输入并确认)",
            device_code_resp.user_code
        );
        info!(
            "🌐 [GitHub Auth] 正在弹出浏览器授权地址: {}",
            device_code_resp.verification_uri
        );

        // 自动使用系统默认浏览器打开授权网页
        if let Err(e) = open::that(&device_code_resp.verification_uri) {
            warn!("未能自动打开浏览器，请手动访问: {} (错误: {})", device_code_resp.verification_uri, e);
        }

        Ok(device_code_resp)
    }

    /// 轮询等待用户在浏览器中授权完成
    pub async fn poll_for_token(
        &mut self,
        client_id: &str,
        device_resp: &GitHubDeviceCodeResponse,
    ) -> Result<String> {
        let interval_secs = if device_resp.interval == 0 { 5 } else { device_resp.interval };
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(device_resp.expires_in);

        info!("⏳ [GitHub Auth] 等待用户在浏览器完成授权点击中...");

        loop {
            interval.tick().await;

            if start.elapsed() > timeout {
                anyhow::bail!("GitHub 授权等待超时，请重新发起！");
            }

            let res = self
                .client
                .post("https://github.com/login/oauth/access_token")
                .header("Accept", "application/json")
                .json(&serde_json::json!({
                    "client_id": client_id,
                    "device_code": device_resp.device_code,
                    "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
                }))
                .send()
                .await?;

            let token_resp: OAuthTokenResponse = res.json().await?;

            if let Some(token) = token_resp.access_token {
                self.config.access_token = Some(token.clone());
                self.config.client_id = Some(client_id.to_string());

                // 校验并获取当前 GitHub 登录用户名
                if let Ok(user) = self.fetch_current_user(&token).await {
                    self.config.username = Some(user);
                }

                self.save_config()?;
                info!(
                    "🎉 [GitHub Auth] 授权成功！当前连接身份: @{}",
                    self.config.username.as_deref().unwrap_or("unknown")
                );
                return Ok(token);
            }

            if let Some(err) = token_resp.error {
                match err.as_str() {
                    "authorization_pending" => {
                        // 用户尚未在浏览器点击，继续等待下一轮 tick
                        continue;
                    }
                    "slow_down" => {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    "expired_token" => {
                        anyhow::bail!("设备码已过期，请重新发起授权！");
                    }
                    "access_denied" => {
                        anyhow::bail!("用户在浏览器中拒绝了授权请求。");
                    }
                    other => {
                        anyhow::bail!("GitHub 授权发生异常: {} ({:?})", other, token_resp.error_description);
                    }
                }
            }
        }
    }

    /// 获取当前登录用户信息
    async fn fetch_current_user(&self, token: &str) -> Result<String> {
        let res = self
            .client
            .get("https://api.github.com/user")
            .bearer_auth(token)
            .send()
            .await?
            .json::<GitHubUserResponse>()
            .await?;
        Ok(res.login)
    }

    /// 安全断开连接并清空配置
    pub fn disconnect(&mut self) -> Result<()> {
        self.config.access_token = None;
        self.config.username = None;
        self.save_config()?;
        info!("已安全断开 GitHub 连接");
        Ok(())
    }

    /// 更新默认绑定的仓库 (如 owner/repo)
    pub fn set_default_repo(&mut self, repo: String) -> Result<()> {
        self.config.default_repo = Some(repo);
        self.save_config()?;
        Ok(())
    }

    /// 手动使用 Personal Access Token (PAT) 直接配置连接
    pub async fn set_personal_access_token(&mut self, token: &str, repo: Option<String>) -> Result<String> {
        let clean_token = token.trim();
        if clean_token.is_empty() {
            anyhow::bail!("Token 不能为空！");
        }
        let user = self.fetch_current_user(clean_token).await.context("验证失败：GitHub Token 无效或网络无法访问 api.github.com")?;
        self.config.access_token = Some(clean_token.to_string());
        self.config.username = Some(user.clone());
        if let Some(r) = repo {
            if !r.trim().is_empty() {
                self.config.default_repo = Some(r.trim().to_string());
            }
        }
        self.save_config()?;
        info!("🎉 [GitHub Auth] PAT 验证通过，已连接到 @{}", user);
        Ok(user)
    }
}
