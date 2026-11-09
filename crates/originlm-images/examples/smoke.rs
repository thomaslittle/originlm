//! Exactly one live request, only when explicitly invoked. Never prints keys.
//! cargo run -p originlm-images --example smoke -- openai ABSOLUTE_ASSET_DIR
use originlm_images::*;
use std::sync::Arc;
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let provider = match args.next().as_deref() {
        Some("openai") => ProviderKind::Openai,
        Some("gemini") => ProviderKind::Gemini,
        Some("openrouter") => ProviderKind::Openrouter,
        _ => return Err("Choose exactly one provider: openai, gemini or openrouter".into()),
    };
    let root = std::path::PathBuf::from(args.next().ok_or("Supply an absolute asset directory")?);
    let route = RouteConfig::from_environment()?.unwrap_or_else(|| RouteConfig::standard(provider));
    if route.provider != provider {
        return Err("Explicit provider differs from configured image route".into());
    }
    let tool = ImageTool::new(Some(Arc::new(route.connect()?)), root, Canvas::default())?;
    let output=tool.generate(GenerateRequest{
        prompt:"Starting Soon broadcast backdrop, no text: moonlit mountain silhouettes, deep midnight blue, subtle warm amber horizon, quiet central negative space.".into(),
        size:None,aspect:None,style:Some("cinematic minimal landscape".into()),reference_image:None,count:1,
    }).await?;
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
