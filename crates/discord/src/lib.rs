use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::types::{Message, Role};
use serenity::async_trait;
use serenity::model::channel::Message as DiscordMessage;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::sync::Arc;

use tracing::{error, info};
use uuid::Uuid;

struct Handler {
    bus: Arc<MessageBus>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: DiscordMessage) {
        if msg.author.bot {
            return;
        }

        let session_key = format!("discord:{}", msg.channel_id);
        info!("Received discord message from channel {}", msg.channel_id);

        let inbound = Message {
            id: Uuid::new_v4(),
            channel: "discord".to_string(),
            session_key: session_key.clone(),
            content: msg.content.clone(),
            role: Role::User,
            metadata: Default::default(),
        };

        if let Err(e) = self.bus.publish(Event::InboundMessage(inbound)) {
            error!("Failed to publish discord message: {}", e);
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

pub struct DiscordBot {
    bus: Arc<MessageBus>,
    token: String,
}

impl DiscordBot {
    pub fn new(bus: Arc<MessageBus>, token: String) -> Self {
        Self { bus, token }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let handler = Handler {
            bus: self.bus.clone(),
        };

        let mut client = Client::builder(&self.token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| anyhow::anyhow!("Error creating client: {}", e))?;

        let bus = self.bus.clone();
        let http = client.http.clone();
        
        // Spawn outbound message handler
        tokio::spawn(async move {
            let mut rx = bus.subscribe();
            loop {
                if let Ok(event) = rx.recv().await {
                    match event {
                        Event::OutboundMessage(msg) => {
                            if msg.session_key.starts_with("discord:") {
                                let channel_id_str = msg.session_key.strip_prefix("discord:").unwrap();
                                if let Ok(channel_id) = channel_id_str.parse::<u64>() {
                                    let channel_id = serenity::model::id::ChannelId::new(channel_id);
                                    if let Err(e) = channel_id.say(&http, &msg.content).await {
                                        error!("Error sending message to Discord: {}", e);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        if let Err(e) = client.start().await {
            error!("Client error: {}", e);
            return Err(anyhow::anyhow!("Client error: {}", e));
        }

        Ok(())
    }
}
