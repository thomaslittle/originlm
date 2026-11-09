//! `originlm image`: the host-owned image tool for SDK and shell callers.
//! Spending quota is the caller's decision, so generation requires `--allow-paid`
//! before any provider is selected or contacted.
use originlm_contracts::{DomainError, ErrorCode, Result};
use originlm_kit::images::{Canvas, ErrorKind, GenerateRequest, ImageError, ImageTool};
use serde_json::json;
use std::{ffi::OsString, path::PathBuf};

fn invalid() -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, "Invalid image arguments")
}

struct Options {
    assets: PathBuf,
    allow_paid: bool,
    request: GenerateRequest,
}

fn text(value: Option<OsString>) -> Result<String> {
    value
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid)
}

fn size(value: &str) -> Result<Canvas> {
    let (width, height) = value.split_once('x').ok_or_else(invalid)?;
    Ok(Canvas {
        width: width.parse().map_err(|_| invalid())?,
        height: height.parse().map_err(|_| invalid())?,
    })
}

fn parse(input: impl IntoIterator<Item = OsString>) -> Result<Option<Options>> {
    let mut input = input.into_iter();
    let (mut assets, mut prompt, mut allow_paid) = (None, None, false);
    let (mut canvas, mut aspect, mut style, mut reference, mut count) = (None, None, None, None, 1);
    while let Some(flag) = input.next() {
        match flag.to_str().ok_or_else(invalid)? {
            "--definition" => {
                return if input.next().is_none() && assets.is_none() && prompt.is_none() {
                    Ok(None)
                } else {
                    Err(invalid())
                };
            }
            "--allow-paid" => allow_paid = true,
            "--assets" => assets = Some(PathBuf::from(input.next().ok_or_else(invalid)?)),
            "--prompt" => prompt = Some(text(input.next())?),
            "--size" => canvas = Some(size(&text(input.next())?)?),
            "--aspect" => aspect = Some(text(input.next())?),
            "--style" => style = Some(text(input.next())?),
            "--reference" => reference = Some(PathBuf::from(input.next().ok_or_else(invalid)?)),
            "--count" => count = text(input.next())?.parse().map_err(|_| invalid())?,
            _ => return Err(invalid()),
        }
    }
    Ok(Some(Options {
        assets: assets.ok_or_else(invalid)?,
        allow_paid,
        request: GenerateRequest {
            prompt: prompt.ok_or_else(invalid)?,
            size: canvas,
            aspect,
            style,
            reference_image: reference,
            count,
        },
    }))
}

fn generate(options: Options) -> std::result::Result<serde_json::Value, ImageError> {
    if !options.allow_paid {
        return Err(ImageError::new(ErrorKind::QuotaBilling));
    }
    let provider = originlm_kit::default_image_provider()?;
    let tool = ImageTool::new(provider, options.assets, Canvas::default())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| ImageError::new(ErrorKind::ProviderFailure))?;
    let images = runtime.block_on(tool.generate(options.request));
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    Ok(json!({ "images": images? }))
}

pub fn run(input: impl IntoIterator<Item = OsString>) -> Result<u8> {
    let Some(options) = parse(input)? else {
        println!("{}", ImageTool::definition());
        return Ok(0);
    };
    match generate(options) {
        Ok(result) => {
            println!("{result}");
            Ok(0)
        }
        Err(error) => {
            println!("{}", json!({ "error": error }));
            Ok(1)
        }
    }
}
