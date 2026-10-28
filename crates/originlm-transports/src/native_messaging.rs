//! Browser framing adapter. No filesystem, keyring, or authorization logic lives here.
use originlm_contracts::*;
use originlm_service::NativeService;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
pub const MAX_MESSAGE: usize = 1_048_576;
fn problem(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Native messaging request failed")
}
pub fn valid_origin(origin: &str) -> bool {
    origin
        .strip_prefix("chrome-extension://")
        .and_then(|s| s.strip_suffix('/'))
        .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)))
}
/// Firefox identifies the caller by extension id (`name@domain`) rather than a
/// `chrome-extension://` origin.
pub fn valid_extension_id(value: &str) -> bool {
    let Some((name, domain)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !domain.is_empty()
        && value.len() <= 128
        && name.len() <= 64
        && domain.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
}
/// Either browser identity shape accepted at the host boundary.
pub fn valid_caller(value: &str) -> bool {
    valid_origin(value) || valid_extension_id(value)
}
async fn exact<R: AsyncRead + Unpin>(r: &mut R, b: &mut [u8]) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), r.read_exact(b))
        .await
        .map_err(|_| problem(ErrorCode::Timeout))?
        .map_err(|_| problem(ErrorCode::ProtocolIncompatible))?;
    Ok(())
}
async fn inbound<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut reader: R,
    mut writer: W,
) -> Result<()> {
    loop {
        let mut header = [0u8; 4];
        let n = reader
            .read(&mut header[..1])
            .await
            .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
        if n == 0 {
            writer
                .shutdown()
                .await
                .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
            return Ok(());
        }
        exact(&mut reader, &mut header[1..]).await?;
        let length = u32::from_ne_bytes(header) as usize;
        if length > MAX_MESSAGE {
            return Err(problem(ErrorCode::FrameTooLarge));
        }
        if length == 0 {
            return Err(problem(ErrorCode::ProtocolIncompatible));
        }
        let mut body = vec![0; length];
        exact(&mut reader, &mut body).await?;
        let value: Value =
            serde_json::from_slice(&body).map_err(|_| problem(ErrorCode::ProtocolIncompatible))?;
        if !value.is_object() {
            return Err(problem(ErrorCode::ProtocolIncompatible));
        }
        let bytes =
            serde_json::to_vec(&value).map_err(|_| problem(ErrorCode::ProtocolIncompatible))?;
        writer
            .write_all(&bytes)
            .await
            .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
    }
}
async fn outbound<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: R,
    mut writer: W,
) -> Result<()> {
    let mut reader = BufReader::new(reader);
    loop {
        let mut line = Vec::new();
        let n = (&mut reader)
            .take((MAX_MESSAGE + 1) as u64)
            .read_until(b'\n', &mut line)
            .await
            .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
        if n == 0 {
            return Ok(());
        }
        if line.pop() != Some(b'\n') {
            return Err(problem(ErrorCode::FrameTooLarge));
        }
        if line.len() > MAX_MESSAGE {
            return Err(problem(ErrorCode::FrameTooLarge));
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            writer.write_all(&(line.len() as u32).to_ne_bytes()).await?;
            writer.write_all(&line).await?;
            writer.flush().await
        })
        .await
        .map_err(|_| problem(ErrorCode::Timeout))?
        .map_err(|_| problem(ErrorCode::ProviderUnavailable))?;
    }
}
pub async fn serve_native<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: R,
    writer: W,
    service: Arc<NativeService>,
    client: String,
    origin: &str,
    allowed: &[String],
) -> Result<()> {
    if !valid_caller(origin) || allowed.len() > 32 || !allowed.iter().any(|v| v == origin) {
        return Err(problem(ErrorCode::LocalAccessDenied));
    }
    let (to_service, service_input) = tokio::io::duplex(65536);
    let (service_output, from_service) = tokio::io::duplex(65536);
    let serving = crate::serve_io(
        service_input,
        service_output,
        service,
        client,
        crate::StdioOptions {
            max_frame_bytes: MAX_MESSAGE,
            ..Default::default()
        },
    );
    tokio::try_join!(
        inbound(reader, to_service),
        outbound(from_service, writer),
        serving
    )?;
    Ok(())
}
