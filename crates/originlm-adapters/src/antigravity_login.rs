//! Where Antigravity keeps each profile's Google sign-in, per operating system.
//!
//! - The `agy` CLI (`~/.gemini/antigravity-cli`) saves its login in its file
//!   store, `antigravity-oauth-token` in the profile, and in the operating
//!   system's credential store through go-keyring (service `gemini`, user
//!   `antigravity`): Windows Credential Manager target `gemini:antigravity`
//!   and the macOS Keychain. The Linux Secret Service entry is not read; it
//!   needs the system D-Bus library, and the file store covers the CLI there.
//! - The current desktop IDE (`~/.gemini/antigravity-ide`) and classic IDE
//!   (`~/.gemini/antigravity`) keep it in their own VS Code state
//!   databases under `Antigravity IDE` and `Antigravity`, respectively, at
//!   `User/globalStorage/state.vscdb`, key
//!   `antigravityUnifiedStateSync.oauthToken`: base64 of a protobuf whose
//!   token message holds the access token and its expiry.
//!
//! - T3 Code's current Google account sign-in uses the CLI file format in
//!   its private profile's `antigravity-acp` directory.
//!
//! Every read is read-only and in memory. Nothing is refreshed or written back:
//! the CLI and the IDE renew their own logins, and an expired one is reported.
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const MAX_LOGIN_BYTES: u64 = 64 * 1024;
const MAX_STATE_DATABASE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const IDE_TOKEN_KEY: &str = "antigravityUnifiedStateSync.oauthToken";
const CLI_TOKEN_FILE: &str = "antigravity-oauth-token";

/// An access token and when it expires (Unix seconds), if stated.
pub struct Login {
    pub access_token: Zeroizing<String>,
    pub expires_at: Option<i64>,
}

impl Login {
    /// Expired, or expiring within 30 seconds of `now`.
    pub fn expired(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|at| at <= now + 30)
    }
}

/// Which Antigravity client owns a profile directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Cli,
    Ide,
    T3,
}

pub fn owner(profile_home: &Path) -> Option<Owner> {
    match profile_home.file_name()?.to_str()? {
        "antigravity-cli" => Some(Owner::Cli),
        "antigravity" | "antigravity-ide" => Some(Owner::Ide),
        key if key.len() == 64
            && key.bytes().all(|byte| byte.is_ascii_hexdigit())
            && profile_home
                .parent()?
                .ends_with("userdata/providers/antigravity") =>
        {
            Some(Owner::T3)
        }
        _ => None,
    }
}

/// Read only the state database belonging to this IDE profile.
pub fn ide_login_for_profile(profile: &Path) -> Option<Login> {
    let app = match profile.file_name()?.to_str()? {
        "antigravity" => "Antigravity",
        "antigravity-ide" => "Antigravity IDE",
        _ => return None,
    };
    let home = profile.parent()?.parent()?;
    let app_data = std::env::var_os("APPDATA").map(PathBuf::from);
    let config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    ide_login(&ide_state_database(
        Os::current(),
        home,
        app_data.as_deref(),
        config_home.as_deref(),
        app,
    ))
}

/// A T3 account is eligible only when the machine has no usable native login.
pub fn has_direct_login(home: &Path, now: i64) -> bool {
    let gemini_home = std::env::var_os("GEMINI_CLI_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".gemini"));
    let cli = gemini_home.join("antigravity-cli");
    (cli.is_dir() && cli_login(&cli).is_some_and(|login| !login.expired(now)))
        || ["antigravity-ide", "antigravity"].into_iter().any(|name| {
            let profile = home.join(".gemini").join(name);
            profile.is_dir()
                && ide_login_for_profile(&profile).is_some_and(|login| !login.expired(now))
        })
}

/// T3's Google account profiles bypass the shared OS credential store.
pub fn t3_login(profile_home: &Path) -> Option<Login> {
    let bytes = Zeroizing::new(read_small(
        &profile_home.join("antigravity-acp").join(CLI_TOKEN_FILE),
    )?);
    let login = parse_cli_login(&bytes)?;
    login.expires_at?;
    Some(login)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }
}

/// The IDE's state database. `app_data` is `%APPDATA%` on Windows and
/// `config_home` is `$XDG_CONFIG_HOME` on Linux; each falls back to its
/// platform default under `home`.
fn ide_state_database(
    os: Os,
    home: &Path,
    app_data: Option<&Path>,
    config_home: Option<&Path>,
    app: &str,
) -> PathBuf {
    let user_data = match os {
        Os::Windows => app_data
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join("AppData").join("Roaming")),
        Os::Macos => home.join("Library").join("Application Support"),
        Os::Linux => config_home
            .filter(|dir| dir.is_absolute())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".config")),
    };
    user_data
        .join(app)
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

/// The login the CLI saved: its file store first, then the OS credential store.
pub fn cli_login(profile_home: &Path) -> Option<Login> {
    read_small(&profile_home.join(CLI_TOKEN_FILE))
        .and_then(|bytes| parse_cli_login(&bytes))
        .or_else(|| os_credential().and_then(|bytes| parse_cli_login(&bytes)))
}

/// The CLI's login JSON (`token.access_token`, `token.expiry`). go-keyring
/// may store it behind a `go-keyring-base64:` or `go-keyring-encoded:` prefix.
pub fn parse_cli_login(bytes: &[u8]) -> Option<Login> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let decoded;
    let json = if let Some(rest) = text.strip_prefix("go-keyring-base64:") {
        use base64::Engine as _;
        decoded = base64::engine::general_purpose::STANDARD
            .decode(rest)
            .ok()?;
        decoded.as_slice()
    } else if let Some(rest) = text.strip_prefix("go-keyring-encoded:") {
        decoded = hex(rest)?;
        decoded.as_slice()
    } else {
        text.as_bytes()
    };
    let value: Value = serde_json::from_slice(json).ok()?;
    let token = value["token"]["access_token"]
        .as_str()
        .filter(|t| !t.is_empty() && t.len() <= 4096)?;
    Some(Login {
        access_token: Zeroizing::new(token.to_owned()),
        expires_at: value["token"]["expiry"]
            .as_str()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|at| at.timestamp()),
    })
}

fn hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(windows)]
fn os_credential() -> Option<Vec<u8>> {
    keyring::Entry::new_with_target("gemini:antigravity", "gemini", "antigravity")
        .ok()?
        .get_secret()
        .ok()
        .filter(|bytes| bytes.len() as u64 <= MAX_LOGIN_BYTES)
}

#[cfg(target_os = "macos")]
fn os_credential() -> Option<Vec<u8>> {
    keyring::Entry::new("gemini", "antigravity")
        .ok()?
        .get_secret()
        .ok()
        .filter(|bytes| bytes.len() as u64 <= MAX_LOGIN_BYTES)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_credential() -> Option<Vec<u8>> {
    None
}

fn read_small(path: &Path) -> Option<Vec<u8>> {
    let info = path.symlink_metadata().ok()?;
    if !info.is_file() || info.len() > MAX_LOGIN_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

/// The IDE's login from its state database, opened read-only.
pub fn ide_login(database: &Path) -> Option<Login> {
    let info = database.symlink_metadata().ok()?;
    if !info.is_file() || info.len() > MAX_STATE_DATABASE_BYTES {
        return None;
    }
    let mut url = reqwest::Url::from_file_path(database).ok()?;
    url.query_pairs_mut().append_pair("mode", "ro");
    let db = Connection::open_with_flags(
        url.as_str(),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    db.busy_timeout(std::time::Duration::from_millis(200))
        .ok()?;
    let value: String = db
        .query_row(
            "SELECT value FROM ItemTable WHERE key=?1 AND length(value) <= 65536 LIMIT 1",
            [IDE_TOKEN_KEY],
            |row| row.get(0),
        )
        .optional()
        .ok()??;
    parse_ide_token(&value)
}

/// `base64(Outer { 1: Entry { 1: "oauthTokenInfoSentinelKey", 2: { 1:
/// base64(Token { 1: access_token, 2: token_type, 3: refresh_token, 4:
/// Timestamp { 1: seconds } }) } } })`. Only the access token and expiry are
/// kept; the refresh token is never copied out of the buffer.
pub fn parse_ide_token(value: &str) -> Option<Login> {
    use base64::Engine as _;
    let base64 = base64::engine::general_purpose::STANDARD;
    let outer = Zeroizing::new(base64.decode(value.trim()).ok()?);
    let entry = protobuf_fields(&outer)?
        .into_iter()
        .filter_map(|(number, field)| (number == 1).then_some(field).flatten())
        .find(|entry| {
            protobuf_bytes(entry, 1).as_deref() == Some(b"oauthTokenInfoSentinelKey".as_slice())
        })?;
    let info = protobuf_bytes(&entry, 2)?;
    let encoded = protobuf_bytes(&info, 1)?;
    let token = Zeroizing::new(base64.decode(std::str::from_utf8(&encoded).ok()?).ok()?);
    let access = protobuf_bytes(&token, 1)?;
    let access = std::str::from_utf8(&access)
        .ok()
        .filter(|t| !t.is_empty() && t.len() <= 4096)?;
    let expires_at = protobuf_bytes(&token, 4)
        .and_then(|timestamp| protobuf_varint(&timestamp, 1))
        .and_then(|seconds| i64::try_from(seconds).ok());
    Some(Login {
        access_token: Zeroizing::new(access.to_owned()),
        expires_at,
    })
}

/// Length-delimited fields as `(number, Some(bytes))`, varints as `(number,
/// None)`. Fails on any other wire type or a truncated buffer.
fn protobuf_fields(buffer: &[u8]) -> Option<Vec<(u64, Option<Vec<u8>>)>> {
    let mut fields = vec![];
    let mut at = 0;
    while at < buffer.len() {
        let key = varint(buffer, &mut at)?;
        match key & 7 {
            0 => {
                varint(buffer, &mut at)?;
                fields.push((key >> 3, None));
            }
            2 => {
                let length = usize::try_from(varint(buffer, &mut at)?).ok()?;
                let end = at.checked_add(length).filter(|end| *end <= buffer.len())?;
                fields.push((key >> 3, Some(buffer[at..end].to_vec())));
                at = end;
            }
            _ => return None,
        }
    }
    Some(fields)
}

fn protobuf_bytes(buffer: &[u8], number: u64) -> Option<Vec<u8>> {
    protobuf_fields(buffer)?
        .into_iter()
        .find_map(|(n, field)| (n == number).then_some(field).flatten())
}

fn protobuf_varint(buffer: &[u8], number: u64) -> Option<u64> {
    let mut at = 0;
    while at < buffer.len() {
        let key = varint(buffer, &mut at)?;
        match key & 7 {
            0 => {
                let value = varint(buffer, &mut at)?;
                if key >> 3 == number {
                    return Some(value);
                }
            }
            2 => {
                let length = usize::try_from(varint(buffer, &mut at)?).ok()?;
                at = at.checked_add(length).filter(|end| *end <= buffer.len())?;
            }
            _ => return None,
        }
    }
    None
}

fn varint(buffer: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *buffer.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn field(number: u64, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![(number << 3 | 2) as u8];
        let mut length = bytes.len();
        loop {
            let byte = (length & 0x7f) as u8;
            length >>= 7;
            if length == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
        out.extend_from_slice(bytes);
        out
    }

    /// The value the IDE writes, built the same way it nests it.
    fn ide_value(access: &str, expiry: u64) -> String {
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut seconds = vec![0x08];
        let mut rest = expiry;
        loop {
            let byte = (rest & 0x7f) as u8;
            rest >>= 7;
            if rest == 0 {
                seconds.push(byte);
                break;
            }
            seconds.push(byte | 0x80);
        }
        let token = [
            field(1, access.as_bytes()),
            field(2, b"Bearer"),
            field(3, b"1//refresh"),
            field(4, &seconds),
        ]
        .concat();
        let info = field(1, b64.encode(token).as_bytes());
        let state = [field(1, b"oauthTokenInfoSentinelKey"), field(2, &info)].concat();
        let auth = [
            field(1, b"authStateWithContextSentinelKey"),
            field(2, &field(1, b"{\"state\":\"signedIn\"}")),
        ]
        .concat();
        b64.encode([field(1, &state), field(1, &auth)].concat())
    }

    fn state_database(dir: &Path, value: &str) -> PathBuf {
        let path = dir.join("state.vscdb");
        let db = Connection::open(&path).unwrap();
        db.execute(
            "CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO ItemTable VALUES (?1, ?2)",
            [IDE_TOKEN_KEY, value],
        )
        .unwrap();
        path
    }

    #[test]
    fn profile_owners_follow_the_directory_name() {
        assert_eq!(
            owner(Path::new("/h/.gemini/antigravity-cli")),
            Some(Owner::Cli)
        );
        assert_eq!(owner(Path::new("/h/.gemini/antigravity")), Some(Owner::Ide));
        assert_eq!(owner(Path::new("/h/.gemini/antigravity-acp")), None);
    }

    #[test]
    fn current_windows_ide_uses_its_own_database() {
        let home = std::env::temp_dir().join("u");
        assert_eq!(
            ide_state_database(Os::Windows, &home, None, None, "Antigravity IDE"),
            home.join("AppData/Roaming/Antigravity IDE/User/globalStorage/state.vscdb")
        );
        assert_eq!(
            owner(Path::new("/h/.gemini/antigravity-ide")),
            Some(Owner::Ide)
        );
    }

    #[test]
    fn ide_state_database_per_os() {
        // Absolute on every host, so XDG_CONFIG_HOME is accepted in the test.
        let base = std::env::temp_dir();
        let home = base.join("u");
        let tail = |root: PathBuf| root.join("Antigravity/User/globalStorage/state.vscdb");
        assert_eq!(
            ide_state_database(Os::Linux, &home, None, None, "Antigravity"),
            tail(home.join(".config"))
        );
        assert_eq!(
            ide_state_database(
                Os::Linux,
                &home,
                None,
                Some(&base.join("conf")),
                "Antigravity"
            ),
            tail(base.join("conf"))
        );
        // A relative XDG_CONFIG_HOME is invalid by the spec and ignored.
        assert_eq!(
            ide_state_database(
                Os::Linux,
                &home,
                None,
                Some(Path::new("conf")),
                "Antigravity"
            ),
            tail(home.join(".config"))
        );
        assert_eq!(
            ide_state_database(Os::Macos, &home, None, None, "Antigravity"),
            tail(home.join("Library/Application Support"))
        );
        assert_eq!(
            ide_state_database(
                Os::Windows,
                &home,
                Some(&base.join("AD")),
                None,
                "Antigravity"
            ),
            tail(base.join("AD"))
        );
        assert_eq!(
            ide_state_database(Os::Windows, &home, None, None, "Antigravity"),
            tail(home.join("AppData/Roaming"))
        );
    }

    #[test]
    fn ide_login_reads_the_token_and_expiry_from_a_state_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = state_database(dir.path(), &ide_value("ya29.ide", 1_792_785_602));
        let login = ide_login(&path).unwrap();
        assert_eq!(login.access_token.as_str(), "ya29.ide");
        assert_eq!(login.expires_at, Some(1_792_785_602));
        assert!(login.expired(1_792_785_602));
        assert!(!login.expired(1_792_785_000));
    }

    #[test]
    fn ide_login_rejects_missing_or_malformed_state() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ide_login(&dir.path().join("state.vscdb")).is_none());
        let path = state_database(dir.path(), "not base64 !");
        assert!(ide_login(&path).is_none());
        assert!(
            parse_ide_token(&base64::engine::general_purpose::STANDARD.encode([0x0a, 0x05]))
                .is_none()
        );
    }

    #[test]
    fn cli_login_reads_the_file_store_and_keyring_encodings() {
        let json = br#"{"token":{"access_token":"ya29.cli","token_type":"Bearer","expiry":"2026-11-22T18:34:44.9470383-05:00"},"auth_method":"personal"}"#;
        let login = parse_cli_login(json).unwrap();
        assert_eq!(login.access_token.as_str(), "ya29.cli");
        assert_eq!(login.expires_at, Some(1_795_390_484));
        let b64 = base64::engine::general_purpose::STANDARD.encode(json);
        assert_eq!(
            parse_cli_login(format!("go-keyring-base64:{b64}").as_bytes())
                .unwrap()
                .access_token
                .as_str(),
            "ya29.cli"
        );
        let hex: String = json.iter().map(|b| format!("{b:02x}")).collect();
        assert!(parse_cli_login(format!("go-keyring-encoded:{hex}").as_bytes()).is_some());
        assert!(parse_cli_login(br#"{"token":{}}"#).is_none());

        let profile = tempfile::tempdir().unwrap();
        std::fs::write(profile.path().join(CLI_TOKEN_FILE), json).unwrap();
        assert_eq!(
            cli_login(profile.path()).unwrap().access_token.as_str(),
            "ya29.cli"
        );
    }
}
