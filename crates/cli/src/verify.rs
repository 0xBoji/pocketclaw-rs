use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use tracing::info;

/// Verify an LLM provider API key by making a minimal chat completion request.
pub async fn verify_api_key(provider: &str, api_key: &str, api_base: Option<&str>) -> Result<String> {
    let client = Client::new();

    let (url, headers, body) = match provider {
        "openai" | "openrouter" => {
            let base = api_base.unwrap_or("https://api.openai.com/v1");
            let url = format!("{}/chat/completions", base);
            let body = json!({
                "model": "gpt-3.5-turbo",
                "messages": [{"role": "user", "content": "Say 'ok' in one word"}],
                "max_tokens": 5
            });
            (url, vec![("Authorization", format!("Bearer {}", api_key))], body)
        }
        "anthropic" => {
            let url = "https://api.anthropic.com/v1/messages".to_string();
            let body = json!({
                "model": "claude-3-haiku-20240307",
                "max_tokens": 5,
                "messages": [{"role": "user", "content": "Say 'ok' in one word"}]
            });
            (
                url,
                vec![
                    ("x-api-key", api_key.to_string()),
                    ("anthropic-version", "2023-06-01".to_string()),
                ],
                body,
            )
        }
        "google" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key={}",
                api_key
            );
            let body = json!({
                "contents": [{"parts": [{"text": "Say 'ok' in one word"}]}]
            });
            (url, vec![], body)
        }
        "groq" => {
            let url = "https://api.groq.com/openai/v1/chat/completions".to_string();
            let body = json!({
                "model": "llama3-8b-8192",
                "messages": [{"role": "user", "content": "Say 'ok' in one word"}],
                "max_tokens": 5
            });
            (url, vec![("Authorization", format!("Bearer {}", api_key))], body)
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown provider: {}", provider));
        }
    };

    let mut req = client.post(&url).json(&body);
    for (key, value) in &headers {
        req = req.header(*key, value);
    }

    let res = req.send().await?;
    let status = res.status();

    if status.is_success() {
        Ok(format!("✅ {} API key is valid (HTTP {})", provider, status))
    } else {
        let body = res.text().await.unwrap_or_default();
        Err(anyhow::anyhow!(
            "❌ {} API key verification failed (HTTP {}): {}",
            provider,
            status,
            &body[..body.len().min(200)]
        ))
    }
}

/// Verify a Telegram bot token via the getMe API.
pub async fn verify_telegram_token(token: &str) -> Result<String> {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/getMe", token);

    let res = client.get(&url).send().await?;

    if res.status().is_success() {
        let data: serde_json::Value = res.json().await?;
        let bot_name = data["result"]["username"]
            .as_str()
            .unwrap_or("unknown");
        info!("Telegram bot verified: @{}", bot_name);
        Ok(format!("✅ Telegram bot verified: @{}", bot_name))
    } else {
        Err(anyhow::anyhow!("❌ Invalid Telegram bot token"))
    }
}

/// Verify a Discord bot token via the /users/@me API.
pub async fn verify_discord_token(token: &str) -> Result<String> {
    let client = Client::new();
    let url = "https://discord.com/api/v10/users/@me";

    let res = client
        .get(url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?;

    if res.status().is_success() {
        let data: serde_json::Value = res.json().await?;
        let bot_name = data["username"].as_str().unwrap_or("unknown");
        info!("Discord bot verified: {}", bot_name);
        Ok(format!("✅ Discord bot verified: {}", bot_name))
    } else {
        Err(anyhow::anyhow!("❌ Invalid Discord bot token"))
    }
}
