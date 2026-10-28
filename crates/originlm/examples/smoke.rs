//! Manual smoke: `LAI_HELPER_PATH=target/debug/originlm cargo run -p originlm --example smoke`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ai = originlm::LocalAi::start()?;
    let status = ai.status()?;
    println!(
        "mode={:?} providers={:?} models={} revision={}",
        status.mode, status.providers, status.models, status.revision
    );
    Ok(())
}
