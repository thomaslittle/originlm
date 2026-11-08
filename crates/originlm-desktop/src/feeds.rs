//! Optional feed worker for the desktop. Example mode and tests never contact a feed.
use originlm_update::feeds::{AppIdentity, FeedConfig, FeedWorker};
use std::{fs::File, io::Read};
pub fn start(example: bool) -> Option<FeedWorker> {
    if example || cfg!(test) {
        return None;
    }
    let mut config = if let Some(path) = std::env::var_os("ORIGINLM_FEED_CONFIG") {
        // Bad or oversized config fails closed; never silently re-enable requests.
        let mut bytes = Vec::new();
        File::open(path)
            .ok()?
            .take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 16 * 1024 {
            return None;
        }
        serde_json::from_slice::<FeedConfig>(&bytes).ok()?
    } else {
        FeedConfig::default()
    };
    config.app = Some(AppIdentity {
        name: "originlm-desktop".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    });
    FeedWorker::start(config).ok().flatten()
}
