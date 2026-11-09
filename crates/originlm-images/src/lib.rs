//! Host-agnostic image tool. Construction is inert; hosts own billing consent,
//! provider selection, canvas, reference scope and the output directory.
pub mod mcp;
mod native_codex;
mod provider;
use image::{ImageFormat, ImageReader};
pub use native_codex::NativeCodexProvider;
pub use provider::{ApiProvider, ProviderKind, RouteConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    future::Future,
    io::Cursor,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

pub const DESCRIPTION: &str = "Generate real imagery when the user asks for scene backdrops, overlays, thumbnails or other pictures. Include the scene name and purpose, canvas aspect, and collection art direction in prompt. Use reference_image for edits/variations of an existing asset. The host supplies the default canvas and asset directory. After success present the image inline, describe it briefly and offer variations. The host decides placement; never claim the image was added to a scene unless the host confirms it. Generation uses the host-selected native Codex subscription or explicit API route, independently of the conversation model. Never substitute a different provider after an error.";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    NoProvider,
    QuotaBilling,
    ContentRefusal,
    ProviderFailure,
    InvalidInput,
    AssetFailure,
}
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ImageError {
    pub kind: ErrorKind,
    pub message: String,
}
pub type Result<T> = std::result::Result<T, ImageError>;
impl ImageError {
    pub fn new(kind: ErrorKind) -> Self {
        let message = match kind {
            ErrorKind::NoProvider => {
                "No usable image provider is configured. Sign in to the installed Codex client, or configure an explicit image API route."
            }
            ErrorKind::QuotaBilling => {
                "Image generation needs paid-route approval or the provider's quota/billing limit was reached."
            }
            ErrorKind::ContentRefusal => {
                "The image provider refused this request. Revise the prompt or reference image."
            }
            ErrorKind::ProviderFailure => {
                "The image provider failed or returned an invalid image. No other provider was tried."
            }
            ErrorKind::InvalidInput => {
                "Check the prompt, count (1–4), canvas (64–4096 per side), aspect and reference asset path."
            }
            ErrorKind::AssetFailure => {
                "The generated image could not be saved in the host's asset directory."
            }
        };
        Self {
            kind,
            message: message.into(),
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
        }
    }
}
impl Canvas {
    fn valid(self) -> bool {
        (64..=4096).contains(&self.width) && (64..=4096).contains(&self.height)
    }
}
fn one() -> u8 {
    1
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateRequest {
    pub prompt: String,
    #[serde(default)]
    pub size: Option<Canvas>,
    #[serde(default)]
    pub aspect: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    /// Relative path inside the caller's asset directory; never an arbitrary URL.
    #[serde(default)]
    pub reference_image: Option<PathBuf>,
    #[serde(default = "one")]
    pub count: u8,
}
#[derive(Clone)]
pub struct ProviderRequest {
    pub prompt: String,
    pub canvas: Canvas,
    pub count: u8,
    pub reference_png: Option<Vec<u8>>,
}
pub type Generation<'a> = Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>>> + Send + 'a>>;
pub trait ImageProvider: Send + Sync {
    fn provider(&self) -> &str;
    fn model(&self) -> &str;
    fn generate<'a>(&'a self, request: ProviderRequest) -> Generation<'a>;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub path: PathBuf,
    pub provider: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    /// Provider sizes may differ from the canvas. The host receives an exact
    /// canvas-size image using a centered cover crop (never stretched).
    pub fit: String,
}
#[derive(Clone)]
pub struct ImageTool {
    provider: Option<Arc<dyn ImageProvider>>,
    assets: PathBuf,
    canvas: Canvas,
}
fn invalid() -> ImageError {
    ImageError::new(ErrorKind::InvalidInput)
}
fn failed() -> ImageError {
    ImageError::new(ErrorKind::ProviderFailure)
}
pub(crate) fn decode(bytes: &[u8]) -> Result<image::DynamicImage> {
    if bytes.is_empty() || bytes.len() > 24 * 1024 * 1024 {
        return Err(failed());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| failed())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| failed())
}
impl ImageTool {
    pub fn new(
        provider: Option<Arc<dyn ImageProvider>>,
        assets: PathBuf,
        canvas: Canvas,
    ) -> Result<Self> {
        if !assets.is_absolute() || !canvas.valid() {
            return Err(invalid());
        }
        std::fs::create_dir_all(&assets).map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
        let assets = assets
            .canonicalize()
            .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
        Ok(Self {
            provider,
            assets,
            canvas,
        })
    }
    pub fn definition() -> Value {
        json!({"name":"generate_image", "description":DESCRIPTION, "inputSchema":{
        "type":"object", "additionalProperties":false, "required":["prompt"], "properties":{
            "prompt":{"type":"string","minLength":1,"maxLength":16000},
            "size":{"type":"object","additionalProperties":false,"required":["width","height"],"properties":{"width":{"type":"integer","minimum":64,"maximum":4096},"height":{"type":"integer","minimum":64,"maximum":4096}}},
            "aspect":{"type":"string","description":"W:H; omit for caller canvas. Must agree with size if both provided."},
            "style":{"type":"string","maxLength":2000},
            "reference_image":{"type":"string","description":"Relative path within the host asset directory."},
            "count":{"type":"integer","default":1,"minimum":1,"maximum":4}
        }}})
    }
    pub async fn generate(&self, request: GenerateRequest) -> Result<Vec<GeneratedImage>> {
        if request.prompt.trim().is_empty()
            || request.prompt.len() > 16000
            || !(1..=4).contains(&request.count)
            || request.style.as_ref().is_some_and(|s| s.len() > 2000)
        {
            return Err(invalid());
        }
        let mut canvas = request.size.unwrap_or(self.canvas);
        if let Some(aspect) = request.aspect.as_deref() {
            let (w, h) = aspect.split_once(':').ok_or_else(invalid)?;
            let w: u32 = w.parse().map_err(|_| invalid())?;
            let h: u32 = h.parse().map_err(|_| invalid())?;
            if w == 0 || h == 0 || w > 100 || h > 100 {
                return Err(invalid());
            }
            if request.size.is_some() {
                if canvas.width as u64 * h as u64 != canvas.height as u64 * w as u64 {
                    return Err(invalid());
                }
            } else if w >= h {
                canvas.height = ((canvas.width as u64 * h as u64) / w as u64) as u32;
            } else {
                canvas.width = ((canvas.height as u64 * w as u64) / h as u64) as u32;
            }
        }
        if !canvas.valid() {
            return Err(invalid());
        }
        let provider = self
            .provider
            .as_ref()
            .ok_or_else(|| ImageError::new(ErrorKind::NoProvider))?;
        let reference_png = if let Some(path) = request.reference_image {
            if path.is_absolute()
                || path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(invalid());
            }
            let resolved = self
                .assets
                .join(path)
                .canonicalize()
                .map_err(|_| invalid())?;
            if !resolved.starts_with(&self.assets) {
                return Err(invalid());
            }
            let bytes = read_bounded(&resolved)?;
            let image = decode(&bytes).map_err(|_| invalid())?;
            let mut png = Cursor::new(Vec::new());
            image
                .write_to(&mut png, ImageFormat::Png)
                .map_err(|_| invalid())?;
            if png.get_ref().len() > 20 * 1024 * 1024 {
                return Err(invalid());
            }
            Some(png.into_inner())
        } else {
            None
        };
        let prompt = format!(
            "{}\nCanvas: {}×{}. Compose for this aspect ratio.{}",
            request.prompt,
            canvas.width,
            canvas.height,
            request
                .style
                .map(|s| format!("\nArt direction: {s}"))
                .unwrap_or_default()
        );
        let bytes = provider
            .generate(ProviderRequest {
                prompt,
                canvas,
                count: request.count,
                reference_png,
            })
            .await?;
        if bytes.len() != request.count as usize {
            return Err(failed());
        }
        let staging = tempfile::Builder::new()
            .prefix("generated-")
            .tempdir_in(&self.assets)
            .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
        let mut output = Vec::new();
        for (index, bytes) in bytes.iter().enumerate() {
            let image = decode(bytes)?;
            let (original_width, original_height) = (image.width(), image.height());
            let path = staging.path().join(format!("image-{}.png", index + 1));
            image
                .resize_to_fill(
                    canvas.width,
                    canvas.height,
                    image::imageops::FilterType::Lanczos3,
                )
                .save_with_format(&path, ImageFormat::Png)
                .map_err(|_| ImageError::new(ErrorKind::AssetFailure))?;
            output.push(GeneratedImage {
                path,
                provider: provider.provider().into(),
                model: provider.model().into(),
                width: canvas.width,
                height: canvas.height,
                original_width,
                original_height,
                fit: "cover_center".into(),
            });
        }
        // Until all files are decoded and written, TempDir rolls back on error.
        let _ = staging.keep();
        Ok(output)
    }
}
fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| invalid())?;
    let mut bytes = Vec::new();
    file.take(20 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(invalid());
    }
    Ok(bytes)
}
