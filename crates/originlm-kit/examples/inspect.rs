use originlm_kit::{ClientPolicy, Kit};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--home" {
        println!("Usage: cargo run -p originlm-kit --example inspect -- --home ABSOLUTE_DIRECTORY");
        return Ok(());
    }
    let kit = Kit::builder().home(args[1].clone().into()).build()?;
    let app = kit.connect("rust-example", ClientPolicy::passive())?;
    let snapshot = app.scan().await?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    app.close();
    Ok(())
}
