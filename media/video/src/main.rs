#![recursion_limit = "512"]
use fframes::{EncoderOptions, RenderOptions, MediaDirectory, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig,
    vulkan::SkiaVulkanCtx,
};
use originlm_film::{ OriginlmFilmVideo, HEIGHT, WIDTH };
use fframes::cli::clap; // the derive below expands to `clap::...`
use std::process::ExitCode;

/// Flags of this video next to the standard ones of `fframes::cli` (render, frame, strip,
/// inspect, audio, ...). Run `cargo run --release -- --help`.
#[derive(Debug, clap::Args)]
struct VideoArgs {
    #[arg(long, default_value = "Originlm Film", global = true)]
    title: String,
}

fn main() -> ExitCode {
    let args = cli::parse::<VideoArgs>();
    let directory = MediaDirectory::read_folder(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("media")).expect("media directory");
    let media = directory.process_media_source().expect("media including the rendered studio.mp4");
    let title = args.app.title.clone();
    let video = OriginlmFilmVideo::new(&title);
    let gpu = SkiaVulkanCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "20"), ("preset", "medium"), ("tune", "animation")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .args(args)
    // Frame previews (frame, strip, onion) render with the same Skia backend.
    .backend(
        SkiaFFramesRenderer::new_vulkan(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    .run()
}
