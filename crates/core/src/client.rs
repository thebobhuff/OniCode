use std::pin::Pin;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    error::{CoreError, Result},
    message::Message,
    tool::ToolInfo,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    Openai,
    Google,
    Bedrock,
    Vertex,
    Ollama,
    OpenRouter,
    HuggingFace,
    Zai,
    Alibaba,
    Xai,
    Custom,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Anthropic => write!(f, "anthropic"),
            LlmProvider::Openai => write!(f, "openai"),
            LlmProvider::Google => write!(f, "google"),
            LlmProvider::Bedrock => write!(f, "bedrock"),
            LlmProvider::Vertex => write!(f, "vertex"),
            LlmProvider::Ollama => write!(f, "ollama"),
            LlmProvider::OpenRouter => write!(f, "openrouter"),
            LlmProvider::HuggingFace => write!(f, "huggingface"),
            LlmProvider::Zai => write!(f, "z.ai"),
            LlmProvider::Alibaba => write!(f, "alibaba"),
            LlmProvider::Xai => write!(f, "x.ai"),
            LlmProvider::Custom => write!(f, "custom"),
        }
    }
}

pub struct LlmClient {
    http: Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    pub fn switch_model(&mut self, model: String) {
        self.config.model = model;
    }
}

#[derive(Debug)]
pub struct LlmResponse {
    pub content: String,
    pub thinking: Option<String>,
    pub tool_calls: Vec<crate::message::ToolCall>,
    pub usage: TokenUsage,
    pub stop_reason: String,
}

#[derive(Debug, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

#[async_trait]
pub trait LlmStream: Send {
    async fn next(&mut self) -> Result<Option<StreamEvent>>;
}

#[derive(Debug)]
pub enum StreamEvent {
    Text(String),
    Thinking(String),
    ToolCall(crate::message::ToolCall),
    Done {
        stop_reason: String,
        usage: TokenUsage,
    },
}

impl LlmClient {
    pub async fn chat(&self, messages: &[Message], tools: &[ToolInfo]) -> Result<LlmResponse> {
        match self.config.provider {
            LlmProvider::Anthropic => self.chat_anthropic(messages, tools).await,
            LlmProvider::Openai => self.chat_openai(messages, tools).await,
            LlmProvider::Google => self.chat_google(messages, tools).await,
            LlmProvider::Ollama => self.chat_ollama(messages, tools).await,
            LlmProvider::OpenRouter
            | LlmProvider::Zai
            | LlmProvider::Alibaba
            | LlmProvider::Xai
            | LlmProvider::Custom => self.chat_openai_compatible(messages, tools).await,
            LlmProvider::HuggingFace => self.chat_huggingface(messages, tools).await,
            LlmProvider::Bedrock | LlmProvider::Vertex => Err(CoreError::LlmError(
                "Bedrock and Vertex providers require additional credential setup".into(),
            )),
        }
    }

    async fn chat_anthropic(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<LlmResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".into());

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens.unwrap_or(16000),
            "messages": messages_to_anthropic(messages),
            "thinking": {
                "type": "enabled",
                "budget_tokens": self.config.max_tokens.map(|m| (m as u64).min(128000) / 2).unwrap_or(8000),
            },
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.parameters,
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        let resp = self
            .http
            .post(format!("{base_url}/v1/messages"))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!(
                "Anthropic API error ({status}): {text}"
            )));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_anthropic_response(&json)
    }

    async fn chat_openai(&self, messages: &[Message], tools: &[ToolInfo]) -> Result<LlmResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com".into());

        let mut body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(max) = self.config.max_tokens {
            body["max_tokens"] = json!(max);
        }

        let resp = self
            .http
            .post(format!("{base_url}/v1/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!(
                "OpenAI API error ({status}): {text}"
            )));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_openai_response(&json)
    }

    async fn chat_google(&self, messages: &[Message], tools: &[ToolInfo]) -> Result<LlmResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com".into());

        let mut body = json!({
            "model": self.config.model,
            "contents": messages_to_google(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!([{
                "functionDeclarations": tools.iter().map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                }).collect::<Vec<_>>(),
            }]);
        }

        let resp = self
            .http
            .post(format!(
                "{base_url}/v1beta/models/{}:generateContent?key={}",
                self.config.model, self.config.api_key
            ))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!(
                "Google API error ({status}): {text}"
            )));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_google_response(&json)
    }

    async fn chat_ollama(&self, messages: &[Message], _tools: &[ToolInfo]) -> Result<LlmResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".into());

        let body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
            "stream": false,
        });

        let resp = self
            .http
            .post(format!("{base_url}/api/chat"))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!(
                "Ollama API error ({status}): {text}"
            )));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_ollama_response(&json)
    }

    async fn chat_openai_compatible(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<LlmResponse> {
        let base_url = self.config.base_url.clone().ok_or_else(|| {
            CoreError::LlmError(format!(
                "base_url is required for {} provider. Use --base_url flag or set in config.",
                self.config.provider
            ))
        })?;

        let mut body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(max) = self.config.max_tokens {
            body["max_tokens"] = json!(max);
        }

        let resp = self
            .http
            .post(format!("{base_url}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!("API error ({status}): {text}")));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_openai_response(&json)
    }

    async fn chat_huggingface(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<LlmResponse> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api-inference.huggingface.co".into());

        let mut body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(max) = self.config.max_tokens {
            body["max_tokens"] = json!(max);
        }

        let resp = self
            .http
            .post(format!(
                "{base_url}/models/{}/{}/v1/chat/completions",
                self.config.provider, self.config.model
            ))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Http(e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CoreError::LlmError(format!(
                "HuggingFace API error ({status}): {text}"
            )));
        }

        let json: Value = resp.json().await.map_err(|e| CoreError::Http(e))?;
        parse_openai_response(&json)
    }
}

pub struct StreamingLlmStream {
    inner: Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>,
}

impl StreamingLlmStream {
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<StreamEvent>> + Send + 'static,
    {
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl Stream for StreamingLlmStream {
    type Item = Result<StreamEvent>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl LlmClient {
    pub fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<StreamingLlmStream> {
        match self.config.provider {
            LlmProvider::Anthropic => self.chat_anthropic_stream(messages, tools),
            LlmProvider::Openai => self.chat_openai_stream(messages, tools),
            LlmProvider::Ollama => self.chat_ollama_stream(messages, tools),
            LlmProvider::OpenRouter
            | LlmProvider::Zai
            | LlmProvider::Alibaba
            | LlmProvider::Xai
            | LlmProvider::Custom
            | LlmProvider::HuggingFace => self.chat_openai_stream(messages, tools),
            LlmProvider::Google => self.chat_google_stream(messages, tools),
            LlmProvider::Bedrock | LlmProvider::Vertex => Err(CoreError::LlmError(
                "Streaming not yet supported for Bedrock and Vertex".into(),
            )),
        }
    }

    fn chat_anthropic_stream(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<StreamingLlmStream> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".into());

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens.unwrap_or(16000),
            "messages": messages_to_anthropic(messages),
            "stream": true,
            "thinking": {
                "type": "enabled",
                "budget_tokens": self.config.max_tokens.map(|m| (m as u64).min(128000) / 2).unwrap_or(8000),
            },
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.parameters,
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        let client = self.http.clone();
        let api_key = self.config.api_key.clone();
        let url = format!("{base_url}/v1/messages");

        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            let resp = match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(CoreError::Http(e))).await;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx
                    .send(Err(CoreError::LlmError(format!(
                        "Anthropic API error ({status}): {text}"
                    ))))
                    .await;
                return;
            }

            let mut input_tokens = 0u64;
            let mut output_tokens = 0u64;
            let mut stop_reason = "unknown".to_string();

            let mut stream = resp.bytes_stream().eventsource();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) if event.data == "[DONE]" => break,
                    Ok(event) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                            let event_type = json["type"].as_str().unwrap_or("");
                            match event_type {
                                "content_block_start" => {
                                    if json["content_block"]["type"].as_str() == Some("text") {
                                        if let Some(text) = json["content_block"]["text"].as_str() {
                                            if !text.is_empty() {
                                                let _ = tx
                                                    .send(Ok(StreamEvent::Text(text.to_string())))
                                                    .await;
                                            }
                                        }
                                    } else if json["content_block"]["type"].as_str()
                                        == Some("thinking")
                                    {
                                        if let Some(thinking) =
                                            json["content_block"]["thinking"].as_str()
                                        {
                                            if !thinking.is_empty() {
                                                let _ = tx
                                                    .send(Ok(StreamEvent::Thinking(
                                                        thinking.to_string(),
                                                    )))
                                                    .await;
                                            }
                                        }
                                    } else if json["content_block"]["type"].as_str()
                                        == Some("tool_use")
                                    {
                                        if let (Some(id), Some(name)) = (
                                            json["content_block"]["id"].as_str(),
                                            json["content_block"]["name"].as_str(),
                                        ) {
                                            let _ = tx
                                                .send(Ok(StreamEvent::ToolCall(
                                                    crate::message::ToolCall {
                                                        id: id.to_string(),
                                                        name: name.to_string(),
                                                        input: json["content_block"]["input"]
                                                            .clone(),
                                                    },
                                                )))
                                                .await;
                                        }
                                    }
                                }
                                "content_block_delta" => {
                                    if let Some(text) = json["delta"]["text"].as_str() {
                                        let _ =
                                            tx.send(Ok(StreamEvent::Text(text.to_string()))).await;
                                    }
                                    if let Some(thinking) = json["delta"]["thinking"].as_str() {
                                        let _ = tx
                                            .send(Ok(StreamEvent::Thinking(thinking.to_string())))
                                            .await;
                                    }
                                }
                                "message_delta" => {
                                    if let Some(reason) = json["delta"]["stop_reason"].as_str() {
                                        stop_reason = reason.to_string();
                                    }
                                    if let Some(usage) = json["usage"].as_object() {
                                        if let Some(ot) = usage["output_tokens"].as_u64() {
                                            output_tokens = ot;
                                        }
                                    }
                                }
                                "message_start" => {
                                    if let Some(usage) = json["message"]["usage"].as_object() {
                                        if let Some(it) = usage["input_tokens"].as_u64() {
                                            input_tokens = it;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = tx
                .send(Ok(StreamEvent::Done {
                    stop_reason,
                    usage: TokenUsage {
                        input_tokens,
                        output_tokens,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                    },
                }))
                .await;
        });

        Ok(StreamingLlmStream::new(ReceiverStream::new(rx)))
    }

    fn chat_openai_stream(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<StreamingLlmStream> {
        let base_url = match self.config.provider {
            LlmProvider::Openai => self
                .config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".into()),
            LlmProvider::OpenRouter => "https://openrouter.ai/api/v1".to_string(),
            LlmProvider::HuggingFace => self
                .config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api-inference.huggingface.co".into()),
            LlmProvider::Zai => "https://api.z.ai/v1".to_string(),
            LlmProvider::Alibaba => "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            LlmProvider::Xai => "https://api.x.ai/v1".to_string(),
            LlmProvider::Custom => self
                .config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".to_string()),
            _ => self
                .config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".into()),
        };

        let mut body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = json!(
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            );
        }

        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(max) = self.config.max_tokens {
            body["max_tokens"] = json!(max);
        }

        let client = self.http.clone();
        let api_key = self.config.api_key.clone();
        let url = format!("{base_url}/chat/completions");

        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            let resp = match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", &api_key))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(CoreError::Http(e))).await;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx
                    .send(Err(CoreError::LlmError(format!(
                        "API error ({status}): {text}"
                    ))))
                    .await;
                return;
            }

            let mut input_tokens = 0u64;
            let mut output_tokens = 0u64;
            let mut stop_reason = "unknown".to_string();

            let mut stream = resp.bytes_stream().eventsource();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) if event.data == "[DONE]" => break,
                    Ok(event) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(choices) = json["choices"].as_array() {
                                if let Some(choice) = choices.first() {
                                    if let Some(reason) = choice["finish_reason"].as_str() {
                                        stop_reason = reason.to_string();
                                    }
                                    if let Some(tc) = choice["delta"]["tool_calls"].as_array() {
                                        for tool_call in tc {
                                            if let (Some(id), Some(name)) = (
                                                tool_call["id"].as_str(),
                                                tool_call["function"]["name"].as_str(),
                                            ) {
                                                let input = serde_json::from_str(
                                                    tool_call["function"]["arguments"]
                                                        .as_str()
                                                        .unwrap_or("{}"),
                                                )
                                                .unwrap_or(Value::Null);
                                                let _ = tx
                                                    .send(Ok(StreamEvent::ToolCall(
                                                        crate::message::ToolCall {
                                                            id: id.to_string(),
                                                            name: name.to_string(),
                                                            input,
                                                        },
                                                    )))
                                                    .await;
                                            }
                                        }
                                    }
                                    if let Some(text) = choice["delta"]["content"].as_str() {
                                        let _ =
                                            tx.send(Ok(StreamEvent::Text(text.to_string()))).await;
                                    }
                                }
                            }
                            if let Some(usage) = json["usage"].as_object() {
                                if let Some(it) = usage["prompt_tokens"].as_u64() {
                                    input_tokens = it;
                                }
                                if let Some(ot) = usage["completion_tokens"].as_u64() {
                                    output_tokens = ot;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = tx
                .send(Ok(StreamEvent::Done {
                    stop_reason,
                    usage: TokenUsage {
                        input_tokens,
                        output_tokens,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                    },
                }))
                .await;
        });

        Ok(StreamingLlmStream::new(ReceiverStream::new(rx)))
    }

    fn chat_google_stream(
        &self,
        messages: &[Message],
        tools: &[ToolInfo],
    ) -> Result<StreamingLlmStream> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com".into());

        let mut body = json!({
            "contents": messages_to_google(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!([{
                "functionDeclarations": tools.iter().map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                }).collect::<Vec<_>>(),
            }]);
        }

        let client = self.http.clone();
        let api_key = self.config.api_key.clone();
        let model = self.config.model.clone();
        let url =
            format!("{base_url}/v1beta/models/{model}:streamGenerateContent?alt=sse&key={api_key}");

        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            let resp = match client
                .post(&url)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(CoreError::Http(e))).await;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx
                    .send(Err(CoreError::LlmError(format!(
                        "Google API error ({status}): {text}"
                    ))))
                    .await;
                return;
            }

            let mut input_tokens = 0u64;
            let mut output_tokens = 0u64;

            let mut stream = resp.bytes_stream().eventsource();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(candidates) = json["candidates"].as_array() {
                                if let Some(candidate) = candidates.first() {
                                    if let Some(text) =
                                        candidate["content"]["parts"][0]["text"].as_str()
                                    {
                                        let _ =
                                            tx.send(Ok(StreamEvent::Text(text.to_string()))).await;
                                    }
                                    if let Some(reason) = candidate["finishReason"].as_str() {
                                        let _ = tx
                                            .send(Ok(StreamEvent::Done {
                                                stop_reason: reason.to_string(),
                                                usage: TokenUsage {
                                                    input_tokens,
                                                    output_tokens,
                                                    cache_read_tokens: 0,
                                                    cache_write_tokens: 0,
                                                },
                                            }))
                                            .await;
                                    }
                                }
                            }
                            if let Some(usage) = json["usageMetadata"].as_object() {
                                if let Some(it) = usage["promptTokenCount"].as_u64() {
                                    input_tokens = it;
                                }
                                if let Some(ot) = usage["candidatesTokenCount"].as_u64() {
                                    output_tokens = ot;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(StreamingLlmStream::new(ReceiverStream::new(rx)))
    }

    fn chat_ollama_stream(
        &self,
        messages: &[Message],
        _tools: &[ToolInfo],
    ) -> Result<StreamingLlmStream> {
        let base_url = self
            .config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".into());

        let body = json!({
            "model": self.config.model,
            "messages": messages_to_openai(messages),
            "stream": true,
        });

        let client = self.http.clone();
        let url = format!("{base_url}/api/chat");

        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            let resp = match client
                .post(&url)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(CoreError::Http(e))).await;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx
                    .send(Err(CoreError::LlmError(format!(
                        "Ollama API error ({status}): {text}"
                    ))))
                    .await;
                return;
            }

            let mut stream = resp.bytes_stream().eventsource();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => {
                        if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(text) = json["message"]["content"].as_str() {
                                if !text.is_empty() {
                                    let _ = tx.send(Ok(StreamEvent::Text(text.to_string()))).await;
                                }
                            }
                            if json["done"].as_bool() == Some(true) {
                                let usage = &json;
                                let _ = tx
                                    .send(Ok(StreamEvent::Done {
                                        stop_reason: "stop".to_string(),
                                        usage: TokenUsage {
                                            input_tokens: usage["prompt_eval_count"]
                                                .as_u64()
                                                .unwrap_or(0),
                                            output_tokens: usage["eval_count"]
                                                .as_u64()
                                                .unwrap_or(0),
                                            cache_read_tokens: 0,
                                            cache_write_tokens: 0,
                                        },
                                    }))
                                    .await;
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(StreamingLlmStream::new(ReceiverStream::new(rx)))
    }
}

fn messages_to_anthropic(messages: &[Message]) -> Value {
    let mut _system_msg = None;
    let mut api_messages = Vec::new();

    for msg in messages {
        match msg.role {
            crate::message::MessageRole::System => {
                _system_msg = Some(msg.content.clone());
            }
            crate::message::MessageRole::User => {
                api_messages.push(json!({ "role": "user", "content": msg.content }));
            }
            crate::message::MessageRole::Assistant => {
                if let Some(ref tool_calls) = msg.tool_calls {
                    let content: Vec<Value> = if !msg.content.is_empty() {
                        vec![json!({ "type": "text", "text": msg.content })]
                    } else {
                        vec![]
                    };
                    let tool_use_blocks: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.input,
                            })
                        })
                        .collect();
                    let mut all_content = content;
                    all_content.extend(tool_use_blocks);
                    api_messages.push(json!({ "role": "assistant", "content": all_content }));
                } else {
                    api_messages.push(json!({ "role": "assistant", "content": msg.content }));
                }
            }
            crate::message::MessageRole::Tool => {
                if let Some(ref result) = msg.tool_result {
                    api_messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": result.tool_call_id,
                            "content": result.content,
                            "is_error": result.is_error,
                        }],
                    }));
                }
            }
        }
    }

    Value::Array(api_messages)
}

fn messages_to_openai(messages: &[Message]) -> Value {
    let api_messages: Vec<Value> = messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                crate::message::MessageRole::User => "user",
                crate::message::MessageRole::Assistant => "assistant",
                crate::message::MessageRole::System => "system",
                crate::message::MessageRole::Tool => "tool",
            };
            let mut obj = json!({
                "role": role,
                "content": msg.content,
            });
            if let Some(ref tool_calls) = msg.tool_calls {
                obj["tool_calls"] = json!(
                    tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.input.to_string(),
                                }
                            })
                        })
                        .collect::<Vec<_>>()
                );
            }
            if let Some(ref result) = msg.tool_result {
                obj["tool_call_id"] = json!(result.tool_call_id);
            }
            obj
        })
        .collect();
    Value::Array(api_messages)
}

fn messages_to_google(messages: &[Message]) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .filter(|m| m.role != crate::message::MessageRole::System)
        .map(|msg| {
            let role = match msg.role {
                crate::message::MessageRole::User => "user",
                crate::message::MessageRole::Assistant => "model",
                _ => "user",
            };
            json!({
                "role": role,
                "parts": [{ "text": msg.content }],
            })
        })
        .collect();
    Value::Array(contents)
}

fn parse_anthropic_response(json: &Value) -> Result<LlmResponse> {
    let content = json["content"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|c| c["type"].as_str() == Some("text"))
                .map(|c| c["text"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let thinking = json["content"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|c| c["type"].as_str() == Some("thinking"))
                .map(|c| c["thinking"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|s| !s.is_empty());

    let tool_calls: Vec<crate::message::ToolCall> = json["content"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|c| c["type"].as_str() == Some("tool_use"))
                .filter_map(|c| {
                    Some(crate::message::ToolCall {
                        id: c["id"].as_str()?.to_string(),
                        name: c["name"].as_str()?.to_string(),
                        input: c["input"].clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let usage = json["usage"].clone();
    let token_usage = TokenUsage {
        input_tokens: usage["input_tokens"].as_u64().unwrap_or(0),
        output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
        cache_read_tokens: usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
        cache_write_tokens: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
    };

    let stop_reason = json["stop_reason"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(LlmResponse {
        content,
        thinking,
        tool_calls,
        usage: token_usage,
        stop_reason,
    })
}

fn parse_openai_response(json: &Value) -> Result<LlmResponse> {
    let choice = &json["choices"][0];
    let message = &choice["message"];

    let content = message["content"].as_str().unwrap_or("").to_string();

    let thinking = message["reasoning_content"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from);

    let tool_calls: Vec<crate::message::ToolCall> = message["tool_calls"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|tc| {
                    Some(crate::message::ToolCall {
                        id: tc["id"].as_str()?.to_string(),
                        name: tc["function"]["name"].as_str()?.to_string(),
                        input: serde_json::from_str(
                            tc["function"]["arguments"].as_str().unwrap_or("{}"),
                        )
                        .unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let usage = &json["usage"];
    let token_usage = TokenUsage {
        input_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: usage["completion_tokens"].as_u64().unwrap_or(0),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    let stop_reason = choice["finish_reason"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(LlmResponse {
        content,
        thinking,
        tool_calls,
        usage: token_usage,
        stop_reason,
    })
}

fn parse_google_response(json: &Value) -> Result<LlmResponse> {
    let candidates = json["candidates"].as_array();
    let content = candidates
        .and_then(|arr| arr.first())
        .and_then(|c| c["content"]["parts"][0]["text"].as_str())
        .unwrap_or("")
        .to_string();

    let usage = json["usageMetadata"].clone();
    let token_usage = TokenUsage {
        input_tokens: usage["promptTokenCount"].as_u64().unwrap_or(0),
        output_tokens: usage["candidatesTokenCount"].as_u64().unwrap_or(0),
        cache_read_tokens: usage["cachedContentTokenCount"].as_u64().unwrap_or(0),
        cache_write_tokens: 0,
    };

    Ok(LlmResponse {
        content,
        thinking: None,
        tool_calls: vec![],
        usage: token_usage,
        stop_reason: "stop".to_string(),
    })
}

fn parse_ollama_response(json: &Value) -> Result<LlmResponse> {
    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let thinking = json["message"]["reasoning"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from);

    let usage = &json;
    let token_usage = TokenUsage {
        input_tokens: usage["prompt_eval_count"].as_u64().unwrap_or(0),
        output_tokens: usage["eval_count"].as_u64().unwrap_or(0),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    Ok(LlmResponse {
        content,
        thinking,
        tool_calls: vec![],
        usage: token_usage,
        stop_reason: "stop".to_string(),
    })
}
