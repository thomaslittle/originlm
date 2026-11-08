use originlm_update::feeds::{AppIdentity, FeedClient, FeedConfig, FeedEnvironment, PollOutcome};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn server(
    release: &'static str,
    catalog: &'static str,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    server_responses(vec![release, catalog])
}
fn server_responses(
    bodies: Vec<&'static str>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let worker = thread::spawn(move || {
        for body in bodies {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = [0; 8192];
            let n = socket.read(&mut bytes).unwrap();
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&bytes[..n]).into_owned());
            if body.starts_with("HTTP/") {
                socket.write_all(body.as_bytes()).unwrap();
            } else {
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
        }
    });
    (base, requests, worker)
}
const RELEASE: &str = r#"{"version":"0.1.0-alpha.27","channel":"alpha","publishedAt":"2026-11-08T12:00:00Z","notes":"A release"}"#;
const CATALOG: &str = r#"{"schemaVersion":1,"generatedAt":"2026-11-08T12:00:00Z","providers":[{"id":"codex","name":"Codex","models":[{"id":"model","name":"Model"}]}]}"#;

#[test]
fn configured_feeds_are_fetched_only_when_due_with_explicit_app_identity() {
    let (base, requests, worker) = server(RELEASE, CATALOG);
    let config = FeedConfig {
        base_url: base,
        interval_seconds: 60,
        app: Some(AppIdentity {
            name: "test-app".into(),
            version: "1.2.3".into(),
        }),
        ..FeedConfig::default()
    };
    let mut client = FeedClient::with_environment(config, FeedEnvironment::default()).unwrap();
    assert!(
        requests.lock().unwrap().is_empty(),
        "construction must be inert"
    );
    let now = Instant::now();
    let PollOutcome::Checked(snapshot) = client.poll_at(now) else {
        panic!("first check is due")
    };
    assert!(snapshot.errors.is_empty());
    assert_eq!(snapshot.release.unwrap().version, "0.1.0-alpha.27");
    assert_eq!(snapshot.catalog.unwrap().providers.len(), 1);
    assert!(matches!(
        client.poll_at(now + Duration::from_secs(59)),
        PollOutcome::Skipped
    ));
    worker.join().unwrap();
    let seen = requests.lock().unwrap();
    assert!(seen[0].starts_with("GET /v1/releases/latest.json "));
    assert!(seen[1].starts_with("GET /v1/catalog.json "));
    assert!(
        seen.iter()
            .all(|r| r.to_lowercase().contains("x-originlm-app: test-app/1.2.3"))
    );
}
#[test]
fn privacy_switches_disable_all_requests_even_with_an_invalid_url() {
    for (enabled, dnt) in [(false, None), (true, Some("1")), (true, Some("true"))] {
        let config = FeedConfig {
            enabled,
            base_url: "invalid".into(),
            ..FeedConfig::default()
        };
        let env = FeedEnvironment {
            do_not_track: dnt.map(str::to_owned),
            ..FeedEnvironment::default()
        };
        let mut client = FeedClient::with_environment(config, env).unwrap();
        assert!(matches!(
            client.poll_at(Instant::now()),
            PollOutcome::Skipped
        ));
    }
}
#[test]
fn base_override_wins_and_bad_identity_is_rejected() {
    assert_eq!(
        FeedConfig::default().base_url,
        "https://originlm.livestreaming.tools"
    );
    let config = FeedConfig {
        base_url: "invalid".into(),
        app: Some(AppIdentity {
            name: "bad\r\nheader".into(),
            version: "1.0.0".into(),
        }),
        ..FeedConfig::default()
    };
    let env = FeedEnvironment {
        base_url: Some("http://127.0.0.1:9".into()),
        ..FeedEnvironment::default()
    };
    assert!(FeedClient::with_environment(config, env).is_err());
}
#[test]
fn malformed_feed_does_not_prevent_the_other_feed_or_cause_a_retry_storm() {
    let (base, _, worker) = server("{}", CATALOG);
    let mut client = FeedClient::with_environment(
        FeedConfig {
            base_url: base,
            ..FeedConfig::default()
        },
        FeedEnvironment::default(),
    )
    .unwrap();
    let now = Instant::now();
    let PollOutcome::Checked(snapshot) = client.poll_at(now) else {
        panic!()
    };
    assert!(snapshot.release.is_none());
    assert!(snapshot.catalog.is_some());
    assert_eq!(snapshot.errors.len(), 1);
    assert!(matches!(
        client.poll_at(now + Duration::from_secs(10)),
        PollOutcome::Skipped
    ));
    worker.join().unwrap();
}

#[test]
fn environment_override_is_used_and_identity_header_is_absent_by_default() {
    let (base, requests, worker) = server(RELEASE, CATALOG);
    let mut client = FeedClient::with_environment(
        FeedConfig {
            base_url: "invalid".into(),
            ..FeedConfig::default()
        },
        FeedEnvironment {
            base_url: Some(base),
            ..FeedEnvironment::default()
        },
    )
    .unwrap();
    let PollOutcome::Checked(snapshot) = client.poll_at(Instant::now()) else {
        panic!()
    };
    assert!(snapshot.errors.is_empty());
    worker.join().unwrap();
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.to_lowercase().contains("x-originlm-app:"))
    );
}
#[test]
fn redirects_are_not_followed_and_other_feed_still_works() {
    let (base, _, worker) = server(
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/should-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        CATALOG,
    );
    let mut client = FeedClient::with_environment(
        FeedConfig {
            base_url: base,
            ..FeedConfig::default()
        },
        FeedEnvironment::default(),
    )
    .unwrap();
    let PollOutcome::Checked(snapshot) = client.poll_at(Instant::now()) else {
        panic!()
    };
    assert_eq!(
        snapshot.errors,
        vec![originlm_update::feeds::FeedError::HttpStatus(302)]
    );
    assert!(snapshot.catalog.is_some());
    worker.join().unwrap();
}
#[test]
fn oversized_content_is_rejected_before_reading_and_invalid_catalog_is_not_adopted() {
    let (base, _, worker) = server(
        "HTTP/1.1 200 OK\r\nContent-Length: 262145\r\nConnection: close\r\n\r\n",
        r#"{"schemaVersion":2,"generatedAt":"2026-11-08T12:00:00Z","providers":[]}"#,
    );
    let mut client = FeedClient::with_environment(
        FeedConfig {
            base_url: base,
            ..FeedConfig::default()
        },
        FeedEnvironment::default(),
    )
    .unwrap();
    let PollOutcome::Checked(snapshot) = client.poll_at(Instant::now()) else {
        panic!()
    };
    assert_eq!(
        snapshot.errors,
        vec![
            originlm_update::feeds::FeedError::TooLarge,
            originlm_update::feeds::FeedError::InvalidFeed
        ]
    );
    assert!(snapshot.catalog.is_none());
    worker.join().unwrap();
}

#[test]
fn documented_examples_match_the_client_contract() {
    use originlm_update::feeds::{CatalogFeed, LatestRelease};
    let release: LatestRelease =
        serde_json::from_str(include_str!("../../../docs/feeds/latest.example.json")).unwrap();
    assert!(semver::Version::parse(&release.version).is_ok());
    let catalog: CatalogFeed =
        serde_json::from_str(include_str!("../../../docs/feeds/catalog.example.json")).unwrap();
    assert_eq!(catalog.schema_version, 1);
}

#[test]
fn update_comparison_uses_semver_and_rejects_invalid_versions() {
    use originlm_update::feeds::LatestRelease;
    let release: LatestRelease = serde_json::from_str(RELEASE).unwrap();
    assert!(release.is_newer_than("0.1.0-alpha.26").unwrap());
    assert!(!release.is_newer_than("0.1.0-alpha.27").unwrap());
    assert!(!release.is_newer_than("0.1.0").unwrap());
    assert!(release.is_newer_than("unknown").is_err());
}

#[test]
fn next_due_poll_retains_last_good_feeds_when_refresh_fails() {
    let (base, requests, worker) = server_responses(vec![RELEASE, CATALOG, "{}", "{}"]);
    let mut client = FeedClient::with_environment(
        FeedConfig {
            base_url: base,
            interval_seconds: 60,
            ..FeedConfig::default()
        },
        FeedEnvironment::default(),
    )
    .unwrap();
    let now = Instant::now();
    let PollOutcome::Checked(first) = client.poll_at(now) else {
        panic!()
    };
    let PollOutcome::Checked(next) = client.poll_at(now + Duration::from_secs(60)) else {
        panic!()
    };
    assert_eq!(next.errors.len(), 2);
    assert_eq!(
        next.release.unwrap().version,
        first.release.unwrap().version
    );
    assert_eq!(
        next.catalog.unwrap().generated_at,
        first.catalog.unwrap().generated_at
    );
    worker.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 4);
}
