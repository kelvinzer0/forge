use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use forge_app::{
    ConversationService, EnvironmentService,
    FileDiscoveryService, ForgeApp, McpConfigManager, ProviderService,
    Services, Walker, WorkflowService, ProviderRegistry, AppConfigService
};
use forge_domain::*;
use forge_infra::ForgeInfra;
use forge_services::{CommandInfra, ForgeServices};
use forge_stream::MpscStream;

use crate::API;

pub struct ForgeAPI<S, F> {
    services: Arc<S>,
    infra: Arc<F>,
}

impl<A, F> ForgeAPI<A, F> {
    pub fn new(services: Arc<A>, infra: Arc<F>) -> Self {
        Self { services, infra }
    }
}

impl ForgeAPI<ForgeServices<ForgeInfra>, ForgeInfra> {
    pub fn init(restricted: bool, cwd: PathBuf) -> Self {
        let infra = Arc::new(ForgeInfra::new(restricted, cwd));
        let app = Arc::new(ForgeServices::new(infra.clone()));
        ForgeAPI::new(app, infra)
    }
}

#[async_trait::async_trait]
impl<A: Services, F: CommandInfra> API for ForgeAPI<A, F> {
    async fn discover(&self) -> Result<Vec<File>> {
        let environment = self.services.get_environment();
        let config = Walker::unlimited().cwd(environment.cwd);
        self.services.collect_files(config).await
    }

    async fn tools(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let forge_app = ForgeApp::new(self.services.clone());
        forge_app.list_tools().await
    }

    async fn models(&self) -> Result<Vec<Model>> {
        Ok(self
            .services
            .models(self.services.get_provider(self.services.read_app_config().await.unwrap_or_default()).await.context("User is not logged in")?)
            .await?)
    }

    async fn chat(
        &self,
        chat: ChatRequest,
    ) -> anyhow::Result<MpscStream<Result<ChatResponse, anyhow::Error>>> {
        // Create a ForgeApp instance and delegate the chat logic to it
        let forge_app = ForgeApp::new(self.services.clone());
        forge_app.chat(chat).await
    }

    async fn init_conversation<W: Into<Workflow> + Send + Sync>(
        &self,
        workflow: W,
    ) -> anyhow::Result<Conversation> {
        self.services.create_conversation(workflow.into()).await
    }

    async fn upsert_conversation(&self, conversation: Conversation) -> anyhow::Result<()> {
        self.services.upsert(conversation).await
    }

    async fn compact_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> anyhow::Result<CompactionResult> {
        let forge_app = ForgeApp::new(self.services.clone());
        forge_app.compact_conversation(conversation_id).await
    }

    fn environment(&self) -> Environment {
        self.services.get_environment().clone()
    }

    async fn read_workflow(&self, path: Option<&Path>) -> anyhow::Result<Workflow> {
        self.services.read_workflow(path).await
    }

    async fn read_merged(&self, path: Option<&Path>) -> anyhow::Result<Workflow> {
        self.services.read_merged(path).await
    }

    async fn write_workflow(&self, path: Option<&Path>, workflow: &Workflow) -> anyhow::Result<()> {
        self.services.write_workflow(path, workflow).await
    }

    async fn update_workflow<T>(&self, path: Option<&Path>, f: T) -> anyhow::Result<Workflow>
    where
        T: FnOnce(&mut Workflow) + Send,
    {
        self.services.update_workflow(path, f).await
    }

    async fn conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> anyhow::Result<Option<Conversation>> {
        self.services.find(conversation_id).await
    }

    async fn execute_shell_command(
        &self,
        command: &str,
        working_dir: PathBuf,
    ) -> anyhow::Result<CommandOutput> {
        self.infra
            .execute_command(command.to_string(), working_dir)
            .await
    }
    async fn read_mcp_config(&self) -> Result<McpConfig> {
        self.services
            .read_mcp_config()
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn write_mcp_config(&self, scope: &Scope, config: &McpConfig) -> Result<()> {
        self.services
            .write_mcp_config(config, scope)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn execute_shell_command_raw(
        &self,
        command: &str,
    ) -> anyhow::Result<std::process::ExitStatus> {
        let cwd = self.environment().cwd;
        self.infra.execute_command_raw(command, cwd).await
    }

    
}
