use crate::context::ContextBuilder;
use crate::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::config::AppConfig;
use pocketclaw_core::types::{Message, Role};
use pocketclaw_providers::{GenerationOptions, LLMProvider};
use pocketclaw_tools::registry::ToolRegistry;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

pub struct AgentLoop {
    bus: Arc<MessageBus>,
    config: AppConfig,
    provider: Arc<dyn LLMProvider>,
    tools: ToolRegistry,
    context_builder: ContextBuilder,
    sessions: SessionManager,
}

impl AgentLoop {
    pub fn new(
        bus: Arc<MessageBus>,
        config: AppConfig,
        provider: Arc<dyn LLMProvider>,
        tools: ToolRegistry,
        context_builder: ContextBuilder,
        sessions: SessionManager,
    ) -> Self {
        Self {
            bus,
            config,
            provider,
            tools,
            context_builder,
            sessions,
        }
    }

    pub async fn run(&self) {
        let mut rx = self.bus.subscribe();

        info!("Agent loop started");

        loop {
            match rx.recv().await {
                Ok(event) => {
                    match event {
                        Event::InboundMessage(msg) => {
                            self.process_message(msg).await;
                        }
                        _ => {}
                    }
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    error!("Agent loop lagged by {} messages", count);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Bus closed, stopping agent loop");
                    break;
                }
            }
        }
    }

    async fn process_message(&self, msg: Message) {
        info!("Processing message: {}", msg.id);

        // 1. Update Session History
        self.sessions.add_message(&msg.session_key, msg.clone()).await;

        // 2. Build Context
        let history = self.sessions.get_history(&msg.session_key).await;
        let summary = self.sessions.get_summary(&msg.session_key).await;
        
        // Note: ContextBuilder::build expects just history and summary, 
        // but here we already added the current message to history.
        // So we might need to adjust ContextBuilder or just pass the full history.
        // Let's assume ContextBuilder takes the full history including the current message.
        // Wait, ContextBuilder::build signature: (history: &[Message], summary: Option<&str>, current_message: &str)
        // If we pass current_message separately, we duplicate it if it's already in history.
        // Let's adjust usage: pass history excluding the last message? 
        // Or better, let's just pass the full history to ContextBuilder and remove the separate `current_message` arg.
        // For now, to match the Plan, I will pass history excluding the new message, and pass the new message explicitly.
        // Actually, easiest is to not add it to history yet?
        // No, we want to persist it.
        // Let's just pop it for the call? No, inefficient.
        // Let's modify ContextBuilder to take full history.
        // Since I already wrote ContextBuilder, let's look at `crates/agent/src/context.rs`.
        // It appends `current_message` at the end.
        // So I should pass `history[..len-1]` to it.
        
        let history_len = history.len();
        let history_slice = if history_len > 0 {
            &history[0..history_len - 1]
        } else {
            &[]
        };

        let messages = self.context_builder.build(
            history_slice,
            summary.as_deref(),
            &msg.content,
        );

        // 3. Prepare Tools
        let tool_defs = self.tools.list_definitions().await;

        // 4. Initial LLM Call
        let options = GenerationOptions {
            model: self.config.agents.default.model.clone(),
            max_tokens: Some(self.config.agents.default.max_tokens),
            temperature: Some(self.config.agents.default.temperature),
        };

        let mut current_messages = messages.clone();
        let mut iteration = 0;
        let max_iterations = 10; 

        while iteration < max_iterations {
            iteration += 1;

            let response = match self.provider.chat(&current_messages, &tool_defs, &options).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!("LLM Provider error: {}", e);
                    return; // Should publish error message back to user?
                }
            };

            // Add assistant response to context
            current_messages.push(Message::new(
                "agent", 
                &msg.session_key, 
                Role::Assistant, 
                &response.content
            ));
            
            // If tool calls present
            if !response.tool_calls.is_empty() {
                for tool_call in response.tool_calls {
                    info!("Executing tool: {}", tool_call.name);
                    
                    let result = if let Some(tool) = self.tools.get(&tool_call.name).await {
                         match serde_json::from_str(&tool_call.arguments) {
                             Ok(args) => match tool.execute(args).await {
                                 Ok(res) => res,
                                 Err(e) => format!("Error executing tool: {}", e),
                             },
                             Err(e) => format!("Error parsing arguments: {}", e),
                         }
                    } else {
                        format!("Tool not found: {}", tool_call.name)
                    };

                    // Add tool result to context
                    // We need a Role::Tool. 
                    // Message::new signature: (channel, session, role, content)
                    // We need to store tool_call_id?
                    // The core::types::Message struct doesn't have tool_call_id field explicitly, 
                    // but it has metadata.
                    // Let's verify `crates/core/src/types.rs`. 
                    // It has `metadata: HashMap<String, String>`.
                    
                    let mut tool_msg = Message::new(
                        "agent",
                        &msg.session_key,
                        Role::Tool,
                        &result,
                    );
                    tool_msg.metadata.insert("tool_call_id".to_string(), tool_call.id);
                    current_messages.push(tool_msg);
                }
                // Loop again to give results back to LLM
                continue;
            }

            // Final Response
            let response_msg = Message::new(
                "agent",
                &msg.session_key,
                Role::Assistant,
                &response.content,
            );
            
            // Update Session
            self.sessions.add_message(&msg.session_key, response_msg.clone()).await;

            // Publish Outbound
            let _ = self.bus.publish(Event::OutboundMessage(response_msg));
            return;
        }
    }
}
