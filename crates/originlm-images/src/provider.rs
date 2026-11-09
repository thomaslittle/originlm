use crate::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use originlm_adapters::{
    http::Endpoint,
    http_provider::{HttpKind, config::EndpointConfig},
};
use originlm_contracts::{DomainError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Openai,
    Gemini,
    Openrouter,
}
/// The endpoint uses OriginLM's existing keyEnv binding. Inline credentials are
/// not configuration. The image route is independent of the text model route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteConfig {
    pub provider: ProviderKind,
    pub endpoint: EndpointConfig,
    #[serde(default)]
    pub model: Option<String>,
}
impl RouteConfig {
    /// Desktop/embedding opt-in: read one explicit, bounded configuration file.
    /// Missing configuration is distinct from malformed configuration.
    pub fn from_environment() -> Result<Option<Self>> {
        use std::io::Read;
        let Some(path) = std::env::var_os("ORIGINLM_IMAGE_ROUTE") else {
            return Ok(None);
        };
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(invalid());
        }
        let file = std::fs::File::open(path).map_err(|_| invalid())?;
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| invalid())
    }
    pub fn default_model(&self) -> &str {
        self.model.as_deref().unwrap_or(match self.provider {
            ProviderKind::Openai => "gpt-image-2.6-flare",
            ProviderKind::Gemini => "gemini-nano-banana-2.1",
            ProviderKind::Openrouter => "google/gemini-nano-banana-2.1",
        })
    }
    /// Only read the one credential named by this user-selected route. Never
    /// enumerate environment values or borrow a native chat OAuth credential.
    pub fn connect(&self) -> Result<ApiProvider> {
        let name = self
            .endpoint
            .key_env
            .as_deref()
            .ok_or_else(|| ImageError::new(ErrorKind::NoProvider))?;
        let credential = std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| ImageError::new(ErrorKind::NoProvider))?;
        self.with_credential(credential)
    }
    pub fn with_credential(&self, credential: String) -> Result<ApiProvider> {
        let expected = match self.provider {
            ProviderKind::Openai => HttpKind::Openai,
            ProviderKind::Gemini => HttpKind::Gemini,
            ProviderKind::Openrouter => HttpKind::Compatible,
        };
        if self.endpoint.kind != expected {
            return Err(invalid());
        }
        // Reuse endpoint config validation, including key-env names and URL rules.
        let doc = json!({"endpoints":[self.endpoint.clone()]}).to_string();
        originlm_adapters::http_provider::config::Registry::parse(&doc, "images")
            .map_err(|_| invalid())?;
        let model = self.default_model();
        if model.is_empty()
            || model.len() > 128
            || !model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b))
        {
            return Err(invalid());
        }
        // The superseded model is rejected explicitly, including dated variants.
        if self.provider == ProviderKind::Openai && model.starts_with("gpt-image-1") {
            return Err(invalid());
        }
        Ok(ApiProvider {
            endpoint: self
                .endpoint
                .endpoint_with_credential(Some(credential))
                .map_err(map_error)?,
            kind: self.provider,
            model: model.into(),
        })
    }
    /// Explicit defaults for hosts which expose a route picker. Selecting a
    /// route never probes availability or switches to another provider.
    pub fn standard(provider: ProviderKind) -> Self {
        let (kind, base, key) = match provider {
            ProviderKind::Openai => (
                HttpKind::Openai,
                "https://api.openai.com/v1/",
                "OPENAI_API_KEY",
            ),
            ProviderKind::Gemini => (
                HttpKind::Gemini,
                "https://generativelanguage.googleapis.com/v1beta/",
                "GEMINI_API_KEY",
            ),
            ProviderKind::Openrouter => (
                HttpKind::Compatible,
                "https://openrouter.ai/api/v1/",
                "OPENROUTER_API_KEY",
            ),
        };
        Self {
            provider,
            model: None,
            endpoint: EndpointConfig {
                id: "images".into(),
                label: "Image generation".into(),
                kind,
                base_url: base.into(),
                key_env: Some(key.into()),
                text_models: vec![],
                provider_billing: Default::default(),
            },
        }
    }
}
/// No Debug: the bound transport contains the user's credential.
pub struct ApiProvider {
    endpoint: Endpoint,
    kind: ProviderKind,
    model: String,
}
fn map_error(error: DomainError) -> ImageError {
    ImageError::new(match error.code {
        ErrorCode::AuthRequired => ErrorKind::NoProvider,
        ErrorCode::BillingConsentRequired | ErrorCode::RateLimited => ErrorKind::QuotaBilling,
        ErrorCode::IntegrationRestricted => ErrorKind::ContentRefusal,
        _ => ErrorKind::ProviderFailure,
    })
}
fn bytes(value: &str) -> Result<Vec<u8>> {
    if value.len() > 32 * 1024 * 1024 {
        return Err(failed());
    }
    STANDARD.decode(value).map_err(|_| failed())
}
fn aspect(canvas: Canvas) -> &'static str {
    let choices = [
        ("1:1", 1., 1.),
        ("16:9", 16., 9.),
        ("9:16", 9., 16.),
        ("4:3", 4., 3.),
        ("3:4", 3., 4.),
        ("3:2", 3., 2.),
        ("2:3", 2., 3.),
        ("21:9", 21., 9.),
        ("5:4", 5., 4.),
        ("4:5", 4., 5.),
    ];
    let ratio = canvas.width as f64 / canvas.height as f64;
    choices
        .into_iter()
        .min_by(|a, b| ((a.1 / a.2 - ratio).abs()).total_cmp(&(b.1 / b.2 - ratio).abs()))
        .unwrap()
        .0
}
impl ImageProvider for ApiProvider {
    fn provider(&self) -> &str {
        match self.kind {
            ProviderKind::Openai => "openai",
            ProviderKind::Gemini => "google",
            ProviderKind::Openrouter => "openrouter",
        }
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn generate<'a>(&'a self, request: ProviderRequest) -> Generation<'a> {
        Box::pin(async move {
            match self.kind {
                ProviderKind::Openai => {
                    let path = if request.reference_png.is_some() {
                        "images/edits"
                    } else {
                        "images/generations"
                    };
                    let body = json!({"model":self.model,"prompt":request.prompt,"n":request.count,"size":format!("{}x{}",request.canvas.width,request.canvas.height),"output_format":"png"});
                    let value = self
                        .endpoint
                        .image_request(path, body, request.reference_png)
                        .await
                        .map_err(map_error)?;
                    let data = value["data"].as_array().ok_or_else(failed)?;
                    data.iter()
                        .map(|v| bytes(v["b64_json"].as_str().ok_or_else(failed)?))
                        .collect()
                }
                ProviderKind::Gemini | ProviderKind::Openrouter => {
                    // Gemini emits one image per call. Count is explicit, capped
                    // at four; a failure aborts the batch, never retries/falls back.
                    let mut output = Vec::new();
                    for _ in 0..request.count {
                        let ratio = aspect(request.canvas);
                        let resolution = if request.canvas.width.max(request.canvas.height) > 2048 {
                            "4K"
                        } else {
                            "2K"
                        };
                        let (path, body) = if self.kind == ProviderKind::Gemini {
                            let mut parts = vec![json!({"text":request.prompt})];
                            if let Some(reference) = &request.reference_png {
                                parts.push(json!({"inlineData":{"mimeType":"image/png","data":STANDARD.encode(reference)}}));
                            }
                            (
                                format!("models/{}:generateContent", self.model),
                                json!({"contents":[{"role":"user","parts":parts}],"generationConfig":{"responseModalities":["TEXT","IMAGE"],"imageConfig":{"aspectRatio":ratio,"imageSize":resolution}}}),
                            )
                        } else {
                            let mut content = vec![json!({"type":"text","text":request.prompt})];
                            if let Some(reference) = &request.reference_png {
                                content.push(json!({"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}",STANDARD.encode(reference))}}));
                            }
                            (
                                "chat/completions".into(),
                                json!({"model":self.model,"messages":[{"role":"user","content":content}],"modalities":["image","text"],"image_config":{"aspect_ratio":ratio,"image_size":resolution}}),
                            )
                        };
                        let value = self
                            .endpoint
                            .image_request(&path, body, None)
                            .await
                            .map_err(map_error)?;
                        if value["promptFeedback"]["blockReason"].is_string()
                            || value["candidates"][0]["finishReason"]
                                .as_str()
                                .is_some_and(|s| {
                                    matches!(s, "SAFETY" | "IMAGE_SAFETY" | "PROHIBITED_CONTENT")
                                })
                            || value["choices"][0]["finish_reason"] == "content_filter"
                            || value["choices"][0]["message"]["refusal"].is_string()
                        {
                            return Err(ImageError::new(ErrorKind::ContentRefusal));
                        }
                        if self.kind == ProviderKind::Gemini {
                            let parts = value["candidates"][0]["content"]["parts"]
                                .as_array()
                                .ok_or_else(failed)?;
                            let data: Vec<_> = parts
                                .iter()
                                .filter_map(|p| {
                                    p.get("inlineData").or_else(|| p.get("inline_data"))
                                })
                                .collect();
                            if data.len() != 1 {
                                return Err(failed());
                            }
                            output.push(bytes(data[0]["data"].as_str().ok_or_else(failed)?)?);
                        } else {
                            let images = value["choices"][0]["message"]["images"]
                                .as_array()
                                .ok_or_else(failed)?;
                            if images.len() != 1 {
                                return Err(failed());
                            }
                            let url = images[0]["image_url"]["url"].as_str().ok_or_else(failed)?;
                            let (_, data) = url
                                .split_once(";base64,")
                                .filter(|(prefix, _)| {
                                    matches!(
                                        *prefix,
                                        "data:image/png" | "data:image/jpeg" | "data:image/webp"
                                    )
                                })
                                .ok_or_else(failed)?;
                            output.push(bytes(data)?);
                        }
                    }
                    Ok(output)
                }
            }
        })
    }
}
