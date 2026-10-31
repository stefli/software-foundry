use anyhow::Result;
use async_trait::async_trait;
use foundry_core::github_auth::{GitHubAuthManager, DEFAULT_GITHUB_CLIENT_ID};
use foundry_core::issue_source::GitHubIssueProvider;
use foundry_core::issue_source::IssueProvider;
use foundry_plugin_api::{FoundryPlugin, PluginKind, ResourceConnectorPlugin};
use foundry_proto::Issue;
use std::io::{self, Write};
use std::path::Path;
use tracing::info;

/// 独立的 GitHub 可插拔资源连接器插件
pub struct GitHubConnectorPlugin {
    #[allow(dead_code)]
    config_dir: std::path::PathBuf,
    auth_mgr: GitHubAuthManager,
    issue_provider: Option<GitHubIssueProvider>,
}

impl GitHubConnectorPlugin {
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        let auth_mgr = GitHubAuthManager::new(&config_dir);
        let issue_provider = if let (Some(token), Some(repo)) = (
            &auth_mgr.config.access_token,
            &auth_mgr.config.default_repo,
        ) {
            Some(GitHubIssueProvider::new(repo.clone(), token.clone()))
        } else {
            None
        };

        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
            auth_mgr,
            issue_provider,
        }
    }
}

#[async_trait]
impl ResourceConnectorPlugin for GitHubConnectorPlugin {
    fn id(&self) -> &'static str {
        "connector-github"
    }

    fn display_name(&self) -> &'static str {
        "GitHub Cloud Repository & Issue Connector"
    }

    fn description(&self) -> &'static str {
        "提供 GitHub OAuth 弹窗设备流授权、仓库代码拉取与 Issues 实时联动"
    }

    async fn is_connected(&self) -> bool {
        self.auth_mgr.is_authenticated()
    }

    /// 弹出浏览器授权并连接 GitHub 资源
    async fn trigger_auth_flow(&mut self) -> Result<()> {
        println!("\n=======================================================");
        println!("🔌 [GitHub 资源连接器插件] 启动授权流程...");
        println!("=======================================================");

        println!("正在向 GitHub 申请设备授权码...");
        let client_id = DEFAULT_GITHUB_CLIENT_ID;
        let device_resp = self.auth_mgr.start_device_flow(Some(client_id.to_string())).await?;

        println!("\n🌐 默认浏览器已自动弹出授权网页！");
        println!("👉 如未自动打开，请手动访问: {}", device_resp.verification_uri);
        println!("🔑 您的授权验证码为: >>> {} <<<", device_resp.user_code);
        println!("(在网页中输入此码并点击 [Authorize / 确认] 即可)\n");

        let token = self.auth_mgr.poll_for_token(client_id, &device_resp).await?;
        let username = self.auth_mgr.config.username.as_deref().unwrap_or("user");

        print!("\n请输入要绑定的 GitHub 仓库名 (如: {}/test-app) [回车默认使用 {}/foundry-test-app]: ", username, username);
        io::stdout().flush()?;
        let mut repo_input = String::new();
        io::stdin().read_line(&mut repo_input)?;
        let repo = repo_input.trim();
        let target_repo = if repo.is_empty() {
            format!("{}/foundry-test-app", username)
        } else {
            repo.to_string()
        };

        self.auth_mgr.config.default_repo = Some(target_repo.clone());
        self.auth_mgr.save_config()?;

        self.issue_provider = Some(GitHubIssueProvider::new(target_repo.clone(), token));
        info!("🎉 GitHub 插件已成功连接仓库: {}", target_repo);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.auth_mgr.config.access_token = None;
        self.auth_mgr.config.username = None;
        self.auth_mgr.save_config()?;
        self.issue_provider = None;
        info!("已安全断开 GitHub 连接");
        Ok(())
    }

    async fn fetch_issues(&self) -> Result<Vec<Issue>> {
        if let Some(ref provider) = self.issue_provider {
            provider.fetch_pending_issues().await
        } else {
            anyhow::bail!("GitHub 连接器尚未连接，请先进行授权！");
        }
    }

    async fn post_comment(&self, issue_id: &str, comment: &str) -> Result<()> {
        if let Some(ref provider) = self.issue_provider {
            provider.update_issue_comment(issue_id, comment).await
        } else {
            anyhow::bail!("GitHub 连接器尚未连接！");
        }
    }
}

impl FoundryPlugin for GitHubConnectorPlugin {
    fn kind(&self) -> PluginKind {
        PluginKind::ResourceConnector
    }
}
