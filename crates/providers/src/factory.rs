use crate::anthropic::AnthropicProvider;
use crate::google::GoogleProvider;
use crate::openai::OpenAIProvider;
use crate::reliable::{FailoverProvider, ReliableProvider};
use crate::LLMProvider;
use phoneclaw_core::config::AppConfig;
use std::sync::Arc;

/// Create the appropriate LLM provider based on the application config.
/// Checks providers in order: OpenAI → OpenRouter → Anthropic → Google.
pub fn create_provider(config: &AppConfig) -> anyhow::Result<Arc<dyn LLMProvider>> {
    let mut providers: Vec<(String, Arc<dyn LLMProvider>, Option<String>)> = Vec::new();

    if let Some(openai_cfg) = &config.providers.openai {
        let p = Arc::new(OpenAIProvider::new(
            openai_cfg.api_key.clone(),
            openai_cfg.api_base.clone(),
        ));
        providers.push((
            "openai".to_string(),
            Arc::new(ReliableProvider::new(p, 2, 250)),
            Some(openai_cfg.model.clone()),
        ));
    }

    if let Some(openrouter_cfg) = &config.providers.openrouter {
        let p = Arc::new(OpenAIProvider::new(
            openrouter_cfg.api_key.clone(),
            openrouter_cfg.api_base.clone(),
        ));
        providers.push((
            "openrouter".to_string(),
            Arc::new(ReliableProvider::new(p, 2, 250)),
            Some(openrouter_cfg.model.clone()),
        ));
    }

    if let Some(anthropic_cfg) = &config.providers.anthropic {
        let p = Arc::new(AnthropicProvider::new(anthropic_cfg.api_key.clone()));
        providers.push((
            "anthropic".to_string(),
            Arc::new(ReliableProvider::new(p, 2, 250)),
            Some(anthropic_cfg.model.clone()),
        ));
    }

    if let Some(google_cfg) = &config.providers.google {
        let p = Arc::new(GoogleProvider::new(
            google_cfg.api_key.clone(),
            google_cfg.model.clone(),
        ));
        providers.push((
            "google".to_string(),
            Arc::new(ReliableProvider::new(p, 2, 250)),
            Some(google_cfg.model.clone()),
        ));
    }

    if let Some(groq_cfg) = &config.providers.groq {
        let p = Arc::new(OpenAIProvider::new(
            groq_cfg.api_key.clone(),
            Some("https://api.groq.com/openai/v1".to_string()),
        ));
        providers.push((
            "groq".to_string(),
            Arc::new(ReliableProvider::new(p, 2, 250)),
            None,
        ));
    }

    if providers.is_empty() {
        anyhow::bail!("No LLM provider configured. Run 'phoneclaw onboard' to set one up.");
    }

    Ok(Arc::new(FailoverProvider::new(providers)))
}
