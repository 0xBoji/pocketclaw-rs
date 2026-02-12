use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::channel::ChannelAdapter;
use pocketclaw_core::types::{Message, Role};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatAction, ParseMode};
use async_trait::async_trait;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct TelegramBot {
    bus: Arc<MessageBus>,
    token: String,
}

impl TelegramBot {
    pub fn new(bus: Arc<MessageBus>, token: String) -> Self {
        Self { bus, token }
    }

    /// Internal start method (called by ChannelAdapter::start)
    async fn start_polling(&self) -> anyhow::Result<()> {
        let bot = Bot::new(&self.token);
        let bus = self.bus.clone();

        info!("Starting Telegram Bot...");

        let handler = Update::filter_message()
            .endpoint(move |_bot: Bot, msg: teloxide::types::Message| {
                let bus = bus.clone();
                async move {
                    // Handle text messages
                    if let Some(text) = msg.text() {
                        let chat_id = msg.chat.id;
                        let sender_id = msg
                            .from()
                            .map(|u| u.id.0.to_string())
                            .unwrap_or_default();
                        let session_key = format!("telegram:{}", chat_id);

                        info!(
                            chat_id = %chat_id,
                            sender_id = %sender_id,
                            "Received telegram message"
                        );

                        let inbound = Message {
                            id: Uuid::new_v4(),
                            channel: "telegram".to_string(),
                            session_key: session_key.clone(),
                            content: text.to_string(),
                            role: Role::User,
                            metadata: Default::default(),
                        };

                        if let Err(e) = bus.publish(Event::InboundMessage(inbound)) {
                            error!("Failed to publish telegram message: {}", e);
                        }
                    }

                    // Handle voice messages
                    if msg.voice().is_some() {
                        let chat_id = msg.chat.id;
                        info!(chat_id = %chat_id, "Received voice message (transcription pending)");

                        let inbound = Message {
                            id: Uuid::new_v4(),
                            channel: "telegram".to_string(),
                            session_key: format!("telegram:{}", chat_id),
                            content: "[Voice message received - transcription not yet integrated]".to_string(),
                            role: Role::User,
                            metadata: Default::default(),
                        };

                        if let Err(e) = bus.publish(Event::InboundMessage(inbound)) {
                            error!("Failed to publish voice message: {}", e);
                        }
                    }

                    // Handle photo messages
                    if let Some(photos) = msg.photo() {
                        let chat_id = msg.chat.id;
                        info!(
                            chat_id = %chat_id,
                            count = photos.len(),
                            "Received photo message"
                        );

                        let caption = msg.caption().unwrap_or("[Photo received]");
                        let inbound = Message {
                            id: Uuid::new_v4(),
                            channel: "telegram".to_string(),
                            session_key: format!("telegram:{}", chat_id),
                            content: caption.to_string(),
                            role: Role::User,
                            metadata: Default::default(),
                        };

                        if let Err(e) = bus.publish(Event::InboundMessage(inbound)) {
                            error!("Failed to publish photo message: {}", e);
                        }
                    }

                    // Handle document messages
                    if msg.document().is_some() {
                        let chat_id = msg.chat.id;
                        let doc_name = msg
                            .document()
                            .and_then(|d| d.file_name.as_deref())
                            .unwrap_or("unknown");

                        info!(chat_id = %chat_id, file = doc_name, "Received document");

                        let caption_default = format!("[Document: {}]", doc_name);
                        let caption = msg.caption().unwrap_or(&caption_default);
                        let inbound = Message {
                            id: Uuid::new_v4(),
                            channel: "telegram".to_string(),
                            session_key: format!("telegram:{}", chat_id),
                            content: caption.to_string(),
                            role: Role::User,
                            metadata: Default::default(),
                        };

                        if let Err(e) = bus.publish(Event::InboundMessage(inbound)) {
                            error!("Failed to publish document message: {}", e);
                        }
                    }

                    respond(())
                }
            });

        // Spawn outbound message handler with "Thinking..." animation
        let bot_clone = bot.clone();
        let bus_clone = self.bus.clone();
        tokio::spawn(async move {
            let mut rx = bus_clone.subscribe();
            loop {
                if let Ok(event) = rx.recv().await {
                    match event {
                        Event::InboundMessage(msg) => {
                            // Send "typing..." indicator when we receive a message
                            if msg.session_key.starts_with("telegram:") {
                                let chat_id_str =
                                    msg.session_key.strip_prefix("telegram:").unwrap();
                                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                    let _ = bot_clone
                                        .send_chat_action(ChatId(chat_id), ChatAction::Typing)
                                        .await;
                                }
                            }
                        }
                        Event::OutboundMessage(msg) => {
                            if msg.session_key.starts_with("telegram:") {
                                let chat_id_str =
                                    msg.session_key.strip_prefix("telegram:").unwrap();
                                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                    // Try with Markdown first, fallback to plain text
                                    let result = bot_clone
                                        .send_message(ChatId(chat_id), &msg.content)
                                        .parse_mode(ParseMode::MarkdownV2)
                                        .await;

                                    if result.is_err() {
                                        warn!("Markdown send failed, falling back to plain text");
                                        let _ = bot_clone
                                            .send_message(ChatId(chat_id), &msg.content)
                                            .await;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }
}

#[async_trait]
impl ChannelAdapter for TelegramBot {
    fn channel_name(&self) -> &str {
        "telegram"
    }

    async fn start(&self) -> anyhow::Result<()> {
        self.start_polling().await
    }
}
