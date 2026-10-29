use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Wire format enumeration for dual-mode networking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodecFormat {
    /// Human-readable JSON format, ideal for debugging and web devtools inspections.
    Json,
    /// Compact binary format powered by Postcard, ideal for production performance.
    Binary,
}

/// Unified codec error type encapsulating JSON and Postcard serialization/deserialization failures.
#[derive(Debug)]
pub enum CodecError {
    /// JSON serialization or deserialization error.
    Json(serde_json::Error),
    /// Postcard binary serialization or deserialization error.
    Postcard(postcard::Error),
    /// Frame format error (e.g., adaptive detection failed or invalid UTF-8).
    Custom(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Json(e) => write!(f, "JSON codec error: {}", e),
            CodecError::Postcard(e) => write!(f, "Postcard codec error: {}", e),
            CodecError::Custom(msg) => write!(f, "Codec error: {}", msg),
        }
    }
}

impl std::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CodecError::Json(e) => Some(e),
            CodecError::Postcard(e) => Some(e),
            CodecError::Custom(_) => None,
        }
    }
}

impl From<serde_json::Error> for CodecError {
    fn from(err: serde_json::Error) -> Self {
        CodecError::Json(err)
    }
}

impl From<postcard::Error> for CodecError {
    fn from(err: postcard::Error) -> Self {
        CodecError::Postcard(err)
    }
}

/// Dual-mode codec supporting JSON text and Postcard binary encoding/decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DualCodec;

impl DualCodec {
    /// Creates a new `DualCodec` instance.
    pub fn new() -> Self {
        Self
    }

    /// Serializes a value to a JSON string using `serde_json`.
    pub fn encode_text<T: Serialize>(val: &T) -> Result<String, CodecError> {
        serde_json::to_string(val).map_err(CodecError::Json)
    }

    /// Deserializes a value from a JSON string using `serde_json`.
    pub fn decode_text<T: DeserializeOwned>(text: &str) -> Result<T, CodecError> {
        serde_json::from_str(text).map_err(CodecError::Json)
    }

    /// Serializes a value to a compact binary byte vector using `postcard`.
    pub fn encode_binary<T: Serialize>(val: &T) -> Result<Vec<u8>, CodecError> {
        postcard::to_allocvec(val).map_err(CodecError::Postcard)
    }

    /// Deserializes a value from binary bytes using `postcard`.
    pub fn decode_binary<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
        postcard::from_bytes(bytes).map_err(CodecError::Postcard)
    }

    /// Encodes a value into a byte buffer according to the given `CodecFormat`.
    /// For `CodecFormat::Json`, returns UTF-8 encoded JSON bytes.
    /// For `CodecFormat::Binary`, returns Postcard binary bytes.
    pub fn encode<T: Serialize>(format: CodecFormat, val: &T) -> Result<Vec<u8>, CodecError> {
        match format {
            CodecFormat::Json => {
                let s = Self::encode_text(val)?;
                Ok(s.into_bytes())
            }
            CodecFormat::Binary => Self::encode_binary(val),
        }
    }

    /// Decodes a value from bytes given an explicit `CodecFormat`.
    pub fn decode<T: DeserializeOwned>(format: CodecFormat, bytes: &[u8]) -> Result<T, CodecError> {
        match format {
            CodecFormat::Json => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|e| CodecError::Custom(format!("Invalid UTF-8 for JSON: {}", e)))?;
                Self::decode_text(text)
            }
            CodecFormat::Binary => Self::decode_binary(bytes),
        }
    }

    /// Adaptively detects format and decodes a value from a byte buffer.
    ///
    /// If the byte sequence begins with `{`, `[`, or `"` (ignoring leading whitespace),
    /// it attempts JSON decoding first, falling back to Postcard binary decoding if unsuccessful.
    /// Otherwise, it attempts Postcard binary decoding first, falling back to JSON.
    pub fn decode_adaptive<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
        let first_non_ws = bytes.iter().find(|&&b| !b.is_ascii_whitespace()).copied();
        let looks_like_json = matches!(first_non_ws, Some(b'{' | b'[' | b'"'));

        if looks_like_json {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if let Ok(val) = Self::decode_text(text) {
                    return Ok(val);
                }
            }
            Self::decode_binary(bytes)
        } else {
            match Self::decode_binary(bytes) {
                Ok(val) => Ok(val),
                Err(postcard_err) => {
                    if let Ok(text) = std::str::from_utf8(bytes) {
                        Self::decode_text(text)
                    } else {
                        Err(postcard_err)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SimpleMsg {
        val: u32,
        name: String,
    }

    #[test]
    fn test_codec_basic_roundtrips() {
        let msg = SimpleMsg {
            val: 42,
            name: "Antigravity".into(),
        };

        // Text (JSON)
        let json_text = DualCodec::encode_text(&msg).unwrap();
        let from_json: SimpleMsg = DualCodec::decode_text(&json_text).unwrap();
        assert_eq!(msg, from_json);

        // Binary (Postcard)
        let bin = DualCodec::encode_binary(&msg).unwrap();
        let from_bin: SimpleMsg = DualCodec::decode_binary(&bin).unwrap();
        assert_eq!(msg, from_bin);
    }

    #[test]
    fn test_format_encoding() {
        let msg = SimpleMsg {
            val: 99,
            name: "Test".into(),
        };

        let j_bytes = DualCodec::encode(CodecFormat::Json, &msg).unwrap();
        let b_bytes = DualCodec::encode(CodecFormat::Binary, &msg).unwrap();

        assert_eq!(
            DualCodec::decode::<SimpleMsg>(CodecFormat::Json, &j_bytes).unwrap(),
            msg
        );
        assert_eq!(
            DualCodec::decode::<SimpleMsg>(CodecFormat::Binary, &b_bytes).unwrap(),
            msg
        );
    }
}
