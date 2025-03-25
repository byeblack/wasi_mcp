use rmcp::{
    Error as McpError, ServerHandler,
    model::{CallToolResult, Content},
    schemars, tool,
};
use serde::{Deserialize, Serialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone)]
pub struct LLM;

#[tool(tool_box)]
impl LLM {
    pub fn new() -> Self {
        Self
    }

    #[tool(description = "LLM Chat")]
    async fn chat(
        &self,
        #[tool(param)] req: crate::llm::ChatRequest,
    ) -> Result<CallToolResult, McpError> {
        tracing::info!("{:?}", req);

        Ok(CallToolResult::success(vec![Content::json(req)?]))
    }

    #[tool(description = "LLM Model List")]
    async fn models(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![]))
    }
}

impl Default for LLM {
    fn default() -> Self {
        Self
    }
}

#[tool(tool_box)]
impl ServerHandler for LLM {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    pub options: Option<ChatOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatResponse {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
}
