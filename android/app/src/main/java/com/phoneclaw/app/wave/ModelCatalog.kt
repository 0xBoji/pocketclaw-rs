package com.phoneclaw.app.wave

object ModelCatalog {
    val providers = arrayOf(
        "openai",
        "anthropic",
        "google",
        "openrouter",
        "groq",
        "xai",
        "zai",
        "mistral",
        "cerebras",
        "moonshot",
        "minimax",
        "qwen-portal",
        "together",
        "synthetic",
        "qianfan",
        "xiaomi"
    )

    val providerModels: Map<String, List<String>> = mapOf(
        "openai" to listOf(
            "gpt-5.2",
            "gpt-5.2-mini",
            "gpt-5.1",
            "gpt-5.1-codex",
            "gpt-5-mini",
            "gpt-4o-mini",
            "gpt-4o",
            "gpt-4.1-mini",
            "gpt-4.1",
            "gpt-4o-mini-transcribe",
            "gpt-4o-audio-preview",
            "text-embedding-3-small"
        ),
        "anthropic" to listOf(
            "claude-opus-4-6",
            "claude-opus-4-5",
            "claude-sonnet-4-5",
            "claude-sonnet-4-5-20250929",
            "claude-sonnet-4",
            "claude-sonnet-4-1",
            "claude-sonnet-4-20250514",
            "claude-haiku-4-5",
            "claude-haiku-4-5-20251001",
            "claude-3-5-sonnet",
            "claude-3-5-sonnet-20241022"
        ),
        "google" to listOf(
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "gemini-3-pro",
            "gemini-2.5-pro-preview",
            "gemini-2.5-flash-preview",
            "gemini-2.0-flash",
            "gemini-2.0-flash-lite",
            "gemini-1.5-flash",
            "gemini-1.5-pro",
            "gemini-embedding-001"
        ),
        "openrouter" to listOf(
            "auto",
            "anthropic/claude-opus-4-5",
            "anthropic/claude-sonnet-4-5",
            "openai/gpt-5.2",
            "openai/gpt-5.2-mini",
            "openai/gpt-4.1",
            "openai/gpt-4o-mini",
            "google/gemini-3-pro-preview",
            "google/gemini-3-flash-preview",
            "google/gemini-2.0-flash-001",
            "google/gemini-2.0-flash-vision:free",
            "meta-llama/llama-3.3-70b-instruct:free",
            "meta-llama/llama-3.3-70b-instruct",
            "meta-llama/llama-3.3-70b:free",
            "qwen/qwen-2.5-vl-72b-instruct:free",
            "deepseek/deepseek-chat-v3-0324",
            "deepseek/deepseek-r1:free",
            "deepseek/deepseek-r1",
            "qwen/qwen-2.5-72b-instruct",
            "moonshotai/kimi-k2",
            "moonshotai/kimi-k2.5",
            "x-ai/grok-4"
        ),
        "groq" to listOf(
            "openai/gpt-oss-120b",
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
            "llama-4-scout-17b-16e-instruct",
            "llama-4-maverick-17b-128e-instruct",
            "gemma2-9b-it",
            "qwen-qwq-32b",
            "deepseek-r1-distill-llama-70b",
            "whisper-large-v3-turbo"
        ),
        "xai" to listOf(
            "grok-4",
            "grok-4-fast-reasoning",
            "grok-3-mini"
        ),
        "zai" to listOf(
            "glm-5",
            "glm-4.7",
            "glm-4.7-flash",
            "glm-4.7-flashx"
        ),
        "mistral" to listOf(
            "mistral-large-latest",
            "mistral-medium-latest",
            "mistral-small-latest",
            "codestral-latest",
            "pixtral-large-latest"
        ),
        "cerebras" to listOf(
            "zai-glm-4.7",
            "zai-glm-4.6",
            "llama-3.3-70b",
            "qwen-3-32b"
        ),
        "moonshot" to listOf(
            "kimi-k2.5",
            "kimi-k2-0905-preview",
            "kimi-k2-turbo-preview",
            "kimi-k2-thinking",
            "kimi-k2-thinking-turbo"
        ),
        "minimax" to listOf(
            "MiniMax-M2.1",
            "MiniMax-M2.1-lightning",
            "MiniMax-VL-01",
            "MiniMax-M2.5",
            "MiniMax-M2.5-Lightning"
        ),
        "qwen-portal" to listOf(
            "coder-model",
            "vision-model"
        ),
        "together" to listOf(
            "zai-org/GLM-4.7",
            "moonshotai/Kimi-K2.5",
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            "meta-llama/Llama-4-Scout-17B-16E-Instruct",
            "meta-llama/Llama-4-Maverick-17B-128E-Instruct-FP8",
            "deepseek-ai/DeepSeek-V3.1",
            "deepseek-ai/DeepSeek-R1",
            "moonshotai/Kimi-K2-Instruct-0905"
        ),
        "synthetic" to listOf(
            "hf:MiniMaxAI/MiniMax-M2.1",
            "hf:moonshotai/Kimi-K2-Thinking",
            "hf:zai-org/GLM-4.7",
            "hf:deepseek-ai/DeepSeek-R1-0528",
            "hf:deepseek-ai/DeepSeek-V3-0324",
            "hf:deepseek-ai/DeepSeek-V3.1",
            "hf:deepseek-ai/DeepSeek-V3.1-Terminus",
            "hf:deepseek-ai/DeepSeek-V3.2",
            "hf:meta-llama/Llama-3.3-70B-Instruct",
            "hf:meta-llama/Llama-4-Maverick-17B-128E-Instruct-FP8",
            "hf:moonshotai/Kimi-K2-Instruct-0905",
            "hf:moonshotai/Kimi-K2.5",
            "hf:openai/gpt-oss-120b",
            "hf:Qwen/Qwen3-235B-A22B-Instruct-2507",
            "hf:Qwen/Qwen3-Coder-480B-A35B-Instruct",
            "hf:Qwen/Qwen3-VL-235B-A22B-Instruct",
            "hf:zai-org/GLM-4.5",
            "hf:zai-org/GLM-4.6",
            "hf:deepseek-ai/DeepSeek-V3",
            "hf:Qwen/Qwen3-235B-A22B-Thinking-2507"
        ),
        "qianfan" to listOf(
            "deepseek-v3.2",
            "ernie-5.0-thinking-preview"
        ),
        "xiaomi" to listOf(
            "mimo-v2-flash"
        )
    )
}
