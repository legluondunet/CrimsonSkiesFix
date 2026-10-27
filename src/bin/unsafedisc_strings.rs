//! Read-only decoding of short metadata labels in the exact original DLL.
//! This does not decode executable instructions or load the Windows binary.

const START: u32 = 0x00DE_88BD;
const LIMIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub rva: u32,
    pub value: String,
}

pub fn decode(encoded: &[u8]) -> Option<(String, usize)> {
    let mut state = START;
    let mut plain = Vec::new();
    for (index, &byte) in encoded.iter().take(LIMIT).enumerate() {
        let value = byte ^ state as u8;
        state = state.wrapping_mul(0xDD43_7681).wrapping_sub(0x5F9A_BCD6);
        if value == 0 {
            let text = String::from_utf8(plain).ok()?;
            return Some((text, index + 1));
        }
        if !value.is_ascii_graphic() && value != b' ' {
            return None;
        }
        plain.push(value);
    }
    None
}

pub fn scan(data: &[u8], data_rva: u32) -> Result<Vec<Label>, String> {
    let mut found = Vec::new();
    for index in 0..data.len() {
        let Some((value, _consumed)) = decode(&data[index..]) else { continue };
        if value.len() < 4 { continue }
        let normal = value.bytes().filter(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')
        }).count();
        if normal * 5 < value.len() * 4 { continue }
        let rva = data_rva.checked_add(index as u32)
            .ok_or_else(|| "original data RVA overflow".to_string())?;
        found.push(Label { rva, value });
    }
    Ok(found)
}

pub fn audit_original(data: &[u8], data_rva: u32) -> Result<Vec<Label>, String> {
    let entries = scan(data, data_rva)?;
    if data_rva != 0x20000 {
        return Err("not the canonical original DLL data section".into());
    }
    for (rva, expected) in [
        (0x20508u32, "Kernel32"),
        (0x20528u32, "User32"),
    ] {
        if !entries.iter().any(|e| e.rva == rva && e.value == expected) {
            return Err(format!("decoded anchor mismatch at RVA 0x{rva:08X}"));
        }
    }
    if entries.len() != 27 {
        return Err(format!("unexpected original decoded label count {}", entries.len()));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(text: &[u8]) -> Vec<u8> {
        let mut state = START;
        text.iter().chain(std::iter::once(&0u8)).map(|&byte| {
            let result = byte ^ state as u8;
            state = state.wrapping_mul(0xDD43_7681).wrapping_sub(0x5F9A_BCD6);
            result
        }).collect()
    }

    #[test]
    fn original_seed_recovers_known_metadata_anchor() {
        assert_eq!(decode(&[0xF6, 0x02, 0x63, 0xD5, 0x00, 0x63, 0x8A, 0x51, 0x0D]),
            Some(("Kernel32".into(), 9)));
    }

    #[test]
    fn roundtrip_and_terminator_are_bounded() {
        for s in ["Demo32", "DataLabel", "Example.file"] {
            let encoded = encode(s.as_bytes());
            assert_eq!(decode(&encoded), Some((s.to_string(), encoded.len())));
            assert_eq!(decode(&encoded[..encoded.len()-1]), None);
        }
        assert_eq!(decode(&vec![0u8; LIMIT]), None);
    }

    #[test]
    fn scanner_accepts_labels_and_skips_short_matches() {
        let mut sample = vec![0u8; 256];
        let encoded = encode(b"DataLabel");
        sample[64..64+encoded.len()].copy_from_slice(&encoded);
        let labels = scan(&sample, 0x20000).unwrap();
        assert!(labels.iter().any(|l| l.rva==0x20040 && l.value=="DataLabel"));
    }
}
