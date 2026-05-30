//! Decode text files that may be UTF-8 or UTF-16LE (common for MT5 / MetaEditor output).

/// Read bytes as UTF-8, or UTF-16LE when a BOM is present.
pub fn decode_text_file(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let mut u16s = Vec::with_capacity((bytes.len().saturating_sub(2)) / 2);
        for chunk in bytes[2..].chunks_exact(2) {
            u16s.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return Some(String::from_utf16_lossy(&u16s));
    }
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Read a path and decode as UTF-8 or UTF-16LE.
pub fn read_text_file(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    decode_text_file(&bytes).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file is not valid UTF-8 or UTF-16LE",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16le_bom() {
        let s = "hello";
        let mut bytes = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_text_file(&bytes).as_deref(), Some("hello"));
    }

    #[test]
    fn decodes_utf8() {
        assert_eq!(
            decode_text_file(b"plain utf8").as_deref(),
            Some("plain utf8")
        );
    }
}
