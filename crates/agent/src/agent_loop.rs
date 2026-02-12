use crate::context::ContextBuilder;
use crate::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::config::AppConfig;
use pocketclaw_core::types::{Message, Role};
use pocketclaw_providers::{GenerationOptions, LLMProvider};
use pocketclaw_tools::registry::ToolRegistry;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use serde_json::json;
use pocketclaw_core::audit::log_audit_internal;

/// Maximum tool-call loop iterations before the agent stops.
const MAX_ITERATIONS: usize = 10;
/// Number of LLM retry attempts on transient errors.
const LLM_RETRIES: usize = 3;
/// History trim threshold — auto-summarize after this many messages per session.
const HISTORY_TRIM_THRESHOLD: usize = 30;

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
        let mut rx = self.bus.subscribe_inbound();

        info!("Agent loop started");

        loop {
            match rx.recv().await {
                Ok(msg) => {
                    self.process_message(msg).await;
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    error!("Agent loop lagged by {} inbound messages (queue full)", count);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Bus closed, stopping agent loop");
                    break;
                }
            }
        }
    }

    /// Call the LLM with retry + exponential backoff for transient failures.
    async fn call_llm_with_retry(
        &self,
        messages: &[Message],
        tool_defs: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<pocketclaw_providers::GenerationResponse, String> {
        let mut last_error = String::new();

        for attempt in 0..LLM_RETRIES {
            match self.provider.chat(messages, tool_defs, options).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_error = e.to_string();
                    if attempt < LLM_RETRIES - 1 {
                        let delay = std::time::Duration::from_millis(1000 * (1 << attempt));
                        warn!(
                            attempt = attempt + 1,
                            max = LLM_RETRIES,
                            delay_ms = delay.as_millis() as u64,
                            "LLM call failed, retrying: {}",
                            last_error
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error)
    }

    /// Send an error/warning message back to the user via the bus.
    fn send_error(&self, session_key: &str, text: &str) {
        let error_msg = Message::new("agent", session_key, Role::Assistant, text);
        let _ = self.bus.publish(Event::OutboundMessage(error_msg));
    }

    async fn process_message(&self, msg: Message) {
        info!("Processing message: {}", msg.id);

        // 1. Update Session History
        self.sessions.add_message(&msg.session_key, msg.clone()).await;

        // 2. Build Context
        let history = self.sessions.get_history(&msg.session_key).await;
        let summary = self.sessions.get_summary(&msg.session_key).await;
        
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

        // 3. Prepare Tools (Filtered by Permissions)
        let allowed_tools = self.context_builder.get_allowed_tools();
        let tool_defs = self.tools.list_definitions_for_permissions(&allowed_tools).await;

        if allowed_tools.is_empty() {
            warn!("No skills approved (or no tools allowed). Agent is running with 0 tools.");
        }

        // 4. Initial LLM Call
        let options = GenerationOptions {
            model: self.config.agents.default.model.clone(),
            max_tokens: Some(self.config.agents.default.max_tokens),
            temperature: Some(self.config.agents.default.temperature),
        };

        let mut current_messages = messages.clone();
        let mut iteration = 0;

        while iteration < MAX_ITERATIONS {
            iteration += 1;

            let response = match self.call_llm_with_retry(&current_messages, &tool_defs, &options).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!("LLM Provider error after {} retries: {}", LLM_RETRIES, e);
                    self.send_error(
                        &msg.session_key,
                        &format!("⚠️ I encountered an error communicating with the AI provider: {}", e),
                    );
                    return;
                }
            };

            // Log token usage for observability/cost tracking
            if let Some(usage) = &response.usage {
                info!(
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    iteration = iteration,
                    session = %msg.session_key,
                    "LLM token usage"
                );

                log_audit_internal(
                    "llm_completion",
                    &msg.session_key,
                    json!({
                        "model": options.model,
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "iteration": iteration
                    })
                );

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

                    // Enforce Default Deny at execution time (Defense in Depth)
                    if !ToolRegistry::is_tool_allowed(&tool_call.name, &allowed_tools) {
                         warn!("Blocked tool execution: '{}' not in allowed list", tool_call.name);
                         
                         log_audit_internal(
                             "security_violation",
                             &msg.session_key,
                             json!({
                                 "type": "tool_blocked",
                                 "tool": tool_call.name,
                                 "reason": "default_deny"
                             })
                         );

                         // feedback to agent
                         current_messages.push(Message::new(
                            "tool",
                            &msg.session_key,
                            Role::Tool,
                            &format!("Error: Tool '{}' is not authorized by any active skill.", tool_call.name),
                        ));
                        continue;
                    }
                    
                    let start = std::time::Instant::now();
                    let result = if let Some(tool) = self.tools.get(&tool_call.name).await {
                         // Permission guard
                         let allowed_tools: Vec<String> = Vec::new();
                         if !ToolRegistry::is_tool_allowed(&tool_call.name, &allowed_tools) {
                             format!("Permission denied: tool '{}' is not allowed", tool_call.name)
                         } else {
                             match serde_json::from_str(&tool_call.arguments) {
                                 Ok(args) => match tool.execute(args).await {
                                     Ok(res) => res,
                                     Err(e) => format!("Error executing tool: {}", e),
                                 },
                                 Err(e) => format!("Error parsing arguments: {}", e),
                             }
                         }
                    } else {
                        format!("Tool not found: {}", tool_call.name)
                    };
                    let elapsed = start.elapsed();

                    // Track metrics
                    self.tools.record_metrics(
                        &tool_call.name,
                        elapsed.as_millis() as u64,
                        !result.starts_with("Error") && !result.starts_with("Permission denied") && !result.starts_with("Tool not found"),
                    ).await;

                    log_audit_internal(
                        "tool_execution",
                        &msg.session_key,
                        json!({
                            "tool": tool_call.name,
                            "args": tool_call.arguments, // string
                            "output_preview": if result.len() > 200 { &result[..200] } else { &result },
                            "duration_ms": elapsed.as_millis(),
                            "success": !result.starts_with("Error")
                        })
                    );

                    let mut tool_msg = Message::new(
                        "agent",
                        &msg.session_key,
                        Role::Tool,
                        &result,
                    );
                    tool_msg.metadata.insert("tool_call_id".to_string(), tool_call.id);
                    current_messages.push(tool_msg);
                }
                // Loop again to return results to LLM
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

            // Auto-summarize and trim history
            self.maybe_summarize_and_trim(&msg.session_key).await;
            return;
        }

        // Max iterations reached — notify user
        warn!(
            session = %msg.session_key,
            iterations = MAX_ITERATIONS,
            "Agent loop hit max iterations"
        );
        self.send_error(
            &msg.session_key,
            &format!(
                "⚠️ I reached the maximum number of processing steps ({}). My last response may be incomplete. Please try rephrasing your request.",
                MAX_ITERATIONS
            ),
        );
    }

    async fn maybe_summarize_and_trim(&self, session_key: &str) {
        let history = self.sessions.get_history(session_key).await;
        
        if self.sessions.should_summarize(session_key, history.len()) {
            info!(session = %session_key, "Auto-summarizing session history...");

            let system_prompt = "You are a helpful assistant. Summarize the conversation history concisely.";
            let user_prompt = format!("Summarize the following conversation into a concise paragraph:\n\n{}", 
                history.iter().map(|m| format!("{:?}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n")
            );

            let messages = vec![
                Message::new("system", session_key, Role::System, system_prompt),
                Message::new("user", session_key, Role::User, &user_prompt),
            ];
            
            let options = GenerationOptions {
                model: self.config.agents.default.model.clone(),
                max_tokens: Some(500),
                temperature: Some(0.3),
            };

            let tool_defs = vec![]; 

            match self.call_llm_with_retry(&messages, &tool_defs, &options).await {
                Ok(resp) => {
                    let summary = resp.content;
                    self.sessions.set_summary(session_key, summary.clone()).await;
                    self.sessions.mark_summarized(session_key);
                    
                    if let Some(usage) = resp.usage {
                        info!(
                            session = %session_key,
                            input_tokens = usage.input_tokens,
                            output_tokens = usage.output_tokens,
                            "Auto-summary cost"
                        );
                    }
                    
                    // Summarized! Now trim to keep last 10 messages
                    self.sessions.auto_trim_history(session_key, 10).await;
                },
                Err(e) => {
                    error!(session = %session_key, "Failed to auto-summarize: {}", e);
                }
            }
        }
    }
}
