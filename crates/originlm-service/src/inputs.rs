//! Owner- and session-scoped staged turn inputs. Payload bytes are never logged or echoed.
use base64::Engine;
use originlm_contracts::*;
use originlm_runtime::ImageInput;
use std::collections::BTreeMap;

/// Upper bound on staged bytes for one application connection and for the whole helper.
const OWNER_BYTES: usize = 64 * 1024 * 1024;
const TOTAL_BYTES: usize = 256 * 1024 * 1024;
const MAX_BASE64_LEN: usize = MAX_INPUT_IMAGE_BYTES.div_ceil(3) * 4;
// One encoded image plus its JSON-RPC envelope must fit in a native request frame.
const _: () = assert!(MAX_BASE64_LEN + 64 * 1024 < MAX_REQUEST_FRAME_BYTES);

pub(crate) struct StagedInput {
    owner: String,
    session: String,
    image: ImageInput,
}
#[derive(Default)]
pub(crate) struct InputStore {
    inputs: BTreeMap<String, StagedInput>,
}
fn invalid(message: &str) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}
fn capacity(message: &str) -> DomainError {
    DomainError::new(ErrorCode::RateLimited, message)
}
fn unavailable() -> DomainError {
    DomainError::new(ErrorCode::NotFound, "Input is unavailable")
}
/// Decode and validate one image. Dimensions come from the PNG IHDR or JPEG SOF header.
pub(crate) fn decode_image(params: &InputPutParams) -> Result<ImageInput> {
    if let Some(name) = &params.display_name
        && (name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control))
    {
        return Err(invalid("Input display name is invalid"));
    }
    if params.data_base64.is_empty() {
        return Err(invalid("Input image is empty"));
    }
    if params.data_base64.len() > MAX_BASE64_LEN {
        return Err(DomainError::new(
            ErrorCode::FrameTooLarge,
            "Input image exceeds 4 MiB",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(params.data_base64.as_bytes())
        .map_err(|_| invalid("Input image is not valid base64"))?;
    if bytes.len() > MAX_INPUT_IMAGE_BYTES {
        return Err(DomainError::new(
            ErrorCode::FrameTooLarge,
            "Input image exceeds 4 MiB",
        ));
    }
    let (width, height) = match params.media_type {
        InputMediaType::Png => png_dimensions(&bytes),
        InputMediaType::Jpeg => jpeg_dimensions(&bytes),
    }
    .ok_or_else(|| invalid("Input image does not match its media type"))?;
    if width == 0 || height == 0 {
        return Err(invalid("Input image has no pixels"));
    }
    if width.max(height) > MAX_INPUT_IMAGE_EDGE {
        return Err(invalid("Input image edge exceeds 2048 pixels"));
    }
    Ok(ImageInput {
        media_type: params.media_type,
        bytes: bytes.into(),
        width,
        height,
    })
}
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 33 || !bytes.starts_with(SIGNATURE) {
        return None;
    }
    // The first chunk must be a 13-byte IHDR.
    if bytes[8..12] != [0, 0, 0, 13] || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut at = 2;
    loop {
        // Markers may be preceded by any number of 0xFF fill bytes.
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        while *bytes.get(at)? == 0xFF {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        match marker {
            0x01 | 0xD0..=0xD7 => continue,
            // End of image or start of scan before any frame header.
            0xD9 | 0xDA | 0x00 => return None,
            _ => {}
        }
        let length = u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]) as usize;
        if length < 2 || at + length > bytes.len() {
            return None;
        }
        // SOF0..SOF15 carry dimensions; C4 (DHT), C8 (JPG), and CC (DAC) do not.
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            if length < 7 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[at + 3], bytes[at + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]) as u32;
            return Some((width, height));
        }
        at += length;
    }
}
fn input_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Secure input identifier generation failed",
        )
    })?;
    Ok(format!(
        "input-{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}
impl InputStore {
    pub fn put(&mut self, owner: &str, session: &str, image: ImageInput) -> Result<InputPutResult> {
        if self
            .inputs
            .values()
            .filter(|input| input.owner == owner && input.session == session)
            .count()
            >= MAX_INPUTS_PER_SESSION
        {
            return Err(capacity("Session input limit reached"));
        }
        let size = image.bytes.len();
        let owned: usize = self
            .inputs
            .values()
            .filter(|input| input.owner == owner)
            .map(|input| input.image.bytes.len())
            .sum();
        let total: usize = self
            .inputs
            .values()
            .map(|input| input.image.bytes.len())
            .sum();
        if owned + size > OWNER_BYTES || total + size > TOTAL_BYTES {
            return Err(capacity("Staged input capacity reached"));
        }
        let id = input_id()?;
        if self.inputs.contains_key(&id) {
            return Err(capacity("Staged input capacity reached"));
        }
        let result = InputPutResult {
            input_id: id.clone(),
            byte_length: size as u64,
            width: image.width,
            height: image.height,
        };
        self.inputs.insert(
            id,
            StagedInput {
                owner: owner.into(),
                session: session.into(),
                image,
            },
        );
        Ok(result)
    }
    /// Resolve references without consuming them. Foreign or expired IDs are indistinguishable.
    pub fn resolve(
        &self,
        owner: &str,
        session: &str,
        refs: &[TurnInputRef],
    ) -> Result<Vec<ImageInput>> {
        if refs.len() > MAX_INPUTS_PER_TURN {
            return Err(invalid("A turn may reference at most 4 inputs"));
        }
        let mut images = Vec::with_capacity(refs.len());
        for (index, reference) in refs.iter().enumerate() {
            if refs[..index]
                .iter()
                .any(|seen| seen.input_id == reference.input_id)
            {
                return Err(invalid("Turn input references must be unique"));
            }
            let staged = self
                .inputs
                .get(&reference.input_id)
                .filter(|input| input.owner == owner && input.session == session)
                .ok_or_else(unavailable)?;
            images.push(staged.image.clone());
        }
        Ok(images)
    }
    pub fn consume(&mut self, refs: &[TurnInputRef]) {
        for reference in refs {
            self.inputs.remove(&reference.input_id);
        }
    }
    pub fn clear_session(&mut self, owner: &str, session: &str) {
        self.inputs
            .retain(|_, input| !(input.owner == owner && input.session == session));
    }
    pub fn clear_owner(&mut self, owner: &str) {
        self.inputs.retain(|_, input| input.owner != owner);
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.inputs.len()
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 13]);
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(b"IEND");
        bytes.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);
        bytes
    }
    pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8];
        // APP0 JFIF segment that must be skipped.
        bytes.extend_from_slice(&[0xFF, 0xE0, 0, 16]);
        bytes.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        // DHT marker (C4) must not be mistaken for a frame header.
        bytes.extend_from_slice(&[0xFF, 0xC4, 0, 4, 0, 0]);
        bytes.extend_from_slice(&[0xFF, 0xFF, 0xC2, 0, 11, 8]);
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[1, 1, 0x11, 0]);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        bytes
    }
    fn params(media: InputMediaType, bytes: &[u8]) -> InputPutParams {
        InputPutParams {
            session_id: "s".into(),
            media_type: media,
            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            display_name: None,
        }
    }
    #[test]
    fn png_and_jpeg_dimensions_come_from_headers() {
        let image = decode_image(&params(InputMediaType::Png, &png(640, 480))).unwrap();
        assert_eq!((image.width, image.height), (640, 480));
        let image = decode_image(&params(InputMediaType::Jpeg, &jpeg(1024, 2048))).unwrap();
        assert_eq!((image.width, image.height), (1024, 2048));
        assert!(format!("{image:?}").contains("byte_length"));
        assert!(image.data_url().starts_with("data:image/jpeg;base64,/9j/"));
    }
    #[test]
    fn invalid_images_are_rejected_with_typed_errors() {
        let code = |p: InputPutParams| decode_image(&p).unwrap_err().code;
        let mut bad = params(InputMediaType::Png, &png(1, 1));
        bad.data_base64 = "not base64!".into();
        assert_eq!(code(bad), ErrorCode::InvalidRequest);
        // Media type must match the magic bytes.
        assert_eq!(
            code(params(InputMediaType::Jpeg, &png(1, 1))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(params(InputMediaType::Png, &jpeg(1, 1))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(params(InputMediaType::Png, &png(2049, 10))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(params(InputMediaType::Jpeg, &jpeg(10, 2049))),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(params(InputMediaType::Png, &png(0, 10))),
            ErrorCode::InvalidRequest
        );
        // A truncated JPEG without a frame header has no trustworthy dimensions.
        assert_eq!(
            code(params(InputMediaType::Jpeg, &jpeg(10, 10)[..24])),
            ErrorCode::InvalidRequest
        );
        let mut oversize = png(16, 16);
        oversize.resize(MAX_INPUT_IMAGE_BYTES + 1, 0);
        assert_eq!(
            code(params(InputMediaType::Png, &oversize)),
            ErrorCode::FrameTooLarge
        );
        let mut named = params(InputMediaType::Png, &png(1, 1));
        named.display_name = Some("line\nbreak".into());
        assert_eq!(code(named), ErrorCode::InvalidRequest);
        let mut empty = params(InputMediaType::Png, &png(1, 1));
        empty.data_base64.clear();
        assert_eq!(code(empty), ErrorCode::InvalidRequest);
    }
    #[test]
    fn store_is_owner_and_session_scoped_bounded_and_consumed() {
        let image = decode_image(&params(InputMediaType::Png, &png(2, 2))).unwrap();
        let mut store = InputStore::default();
        let first = store.put("app", "s1", image.clone()).unwrap();
        assert_eq!(first.byte_length, png(2, 2).len() as u64);
        let refs = vec![TurnInputRef {
            input_id: first.input_id.clone(),
        }];
        assert_eq!(store.resolve("app", "s1", &refs).unwrap().len(), 1);
        assert_eq!(
            store.resolve("other", "s1", &refs).unwrap_err().code,
            ErrorCode::NotFound
        );
        assert_eq!(
            store.resolve("app", "s2", &refs).unwrap_err().code,
            ErrorCode::NotFound
        );
        let duplicate = vec![refs[0].clone(), refs[0].clone()];
        assert_eq!(
            store.resolve("app", "s1", &duplicate).unwrap_err().code,
            ErrorCode::InvalidRequest
        );
        let five = vec![refs[0].clone(); 5];
        assert_eq!(
            store.resolve("app", "s1", &five).unwrap_err().code,
            ErrorCode::InvalidRequest
        );
        for _ in 1..MAX_INPUTS_PER_SESSION {
            store.put("app", "s1", image.clone()).unwrap();
        }
        assert_eq!(
            store.put("app", "s1", image.clone()).unwrap_err().code,
            ErrorCode::RateLimited
        );
        store.put("app", "s2", image.clone()).unwrap();
        store.consume(&refs);
        assert_eq!(
            store.resolve("app", "s1", &refs).unwrap_err().code,
            ErrorCode::NotFound
        );
        store.clear_session("app", "s1");
        assert_eq!(store.len(), 1);
        store.put("other", "s1", image).unwrap();
        store.clear_owner("app");
        assert_eq!(store.len(), 1);
    }
    #[test]
    fn owner_byte_budget_is_enforced() {
        let mut bytes = png(16, 16);
        bytes.resize(MAX_INPUT_IMAGE_BYTES, 0);
        let image = decode_image(&params(InputMediaType::Png, &bytes)).unwrap();
        let mut store = InputStore::default();
        let per_owner = OWNER_BYTES / MAX_INPUT_IMAGE_BYTES;
        for index in 0..per_owner {
            store
                .put(
                    "app",
                    &format!("s{}", index / MAX_INPUTS_PER_SESSION),
                    image.clone(),
                )
                .unwrap();
        }
        assert_eq!(
            store.put("app", "fresh", image.clone()).unwrap_err().code,
            ErrorCode::RateLimited
        );
        store.put("other", "fresh", image).unwrap();
    }
}
