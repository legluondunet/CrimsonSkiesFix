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

/// A literal PUSH imm32 byte sequence that references decoded metadata.
/// The instruction encoding is verified, but runtime reachability is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReference {
    pub instruction_rva: u32,
    pub label_rva: u32,
}

/// Report whether a candidate PUSH imm32 has an IMAGE_REL_BASED_HIGHLOW
/// relocation on the *operand* (opcode RVA + 1), not on its opcode byte.
/// This strengthens static evidence for an absolute pointer but does not
/// establish instruction boundaries or runtime reachability.
pub fn push_operand_highlow_flags(
    references: &[PushReference],
    relocation_sites: &std::collections::HashSet<u32>,
) -> Result<Vec<bool>, String> {
    references.iter().map(|reference| {
        let operand_rva = reference.instruction_rva.checked_add(1)
            .ok_or_else(|| "PUSH operand RVA overflow".to_string())?;
        Ok(relocation_sites.contains(&operand_rva))
    }).collect()
}

pub fn scan_push_references(
    code: &[u8], code_rva: u32, image_base: u32, labels: &[Label],
) -> Result<Vec<PushReference>, String> {
    let mut pointers = std::collections::HashMap::new();
    for label in labels {
        let va = image_base.checked_add(label.rva)
            .ok_or_else(|| "decoded label VA overflow".to_string())?;
        pointers.insert(va, label.rva);
    }
    let mut found = Vec::new();
    for (index, window) in code.windows(5).enumerate() {
        if window[0] != 0x68 { continue; }
        let value = u32::from_le_bytes(window[1..5].try_into().unwrap());
        if let Some(&label_rva) = pointers.get(&value) {
            let instruction_rva = code_rva.checked_add(index as u32)
                .ok_or_else(|| "code RVA overflow".to_string())?;
            found.push(PushReference { instruction_rva, label_rva });
        }
    }
    Ok(found)
}

/// Find metadata addresses at known PE HIGHLOW slots in a file-backed
/// section. Unlike scanning for 0x68, this also discovers operands stored
/// under other opcode encodings and identifies actual relocatable pointers.
/// The byte *before* the slot is only context, not a disassembled instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocatedLabelPointer {
    pub operand_rva: u32,
    pub label_rva: u32,
    pub previous_byte: Option<u8>,
}

pub fn scan_relocated_label_pointers(
    section_bytes: &[u8],
    section_rva: u32,
    image_base: u32,
    labels: &[Label],
    highlow_sites: &std::collections::HashSet<u32>,
) -> Result<Vec<RelocatedLabelPointer>, String> {
    let mut addresses = std::collections::HashMap::new();
    for label in labels {
        let va = image_base.checked_add(label.rva)
            .ok_or_else(|| "metadata label VA overflow".to_string())?;
        addresses.insert(va, label.rva);
    }
    let mut hits = Vec::new();
    for &site in highlow_sites {
        let Some(relative) = site.checked_sub(section_rva) else { continue };
        let at = usize::try_from(relative)
            .map_err(|_| "HIGHLOW RVA does not fit usize".to_string())?;
        let Some(operand) = section_bytes.get(at..at.saturating_add(4)) else { continue };
        let address = u32::from_le_bytes(operand.try_into().unwrap());
        let Some(&label_rva) = addresses.get(&address) else { continue };
        hits.push(RelocatedLabelPointer {
            operand_rva: site,
            label_rva,
            previous_byte: at.checked_sub(1).and_then(|i| section_bytes.get(i).copied()),
        });
    }
    hits.sort_unstable_by_key(|hit| (hit.operand_rva, hit.label_rva));
    Ok(hits)
}

// Recognize a complete x86 encoded-API resolver sequence:
// PUSH decoded-label VA, PUSH module handle, CALL C708,
// ADD ESP,8, MOV [absolute zero-fill destination],EAX,
// CMP DWORD [same destination],0.
// All three absolute operands must be real HIGHLOW slots.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ResolverSlot {
    pub label_rva:u32,
    pub instruction_rva:u32,
    pub call_rva:u32,
    pub output_slot_rva:u32,
}

pub fn scan_resolver_slots(
    code:&[u8], code_rva:u32, image_base:u32, labels:&[Label],
    relocations:&std::collections::HashSet<u32>, resolver_rva:u32,
)->Result<Vec<ResolverSlot>,String>{
    let mut label_vas=std::collections::HashMap::new();
    for item in labels{
        let va=image_base.checked_add(item.rva).ok_or("label VA overflow")?;
        label_vas.insert(va,item.rva);
    }
    let mut hits=Vec::new();
    for (index,w) in code.windows(31).enumerate(){
        if w[0]!=0x68{continue}
        let va=u32::from_le_bytes(w[1..5].try_into().unwrap());
        let Some(&label_rva)=label_vas.get(&va) else{continue};
        let site=code_rva.checked_add(index as u32).ok_or("site overflow")?;
        if !relocations.contains(&site.checked_add(1).ok_or("push overflow")?){continue}
        let prelude=if matches!(&w[5..9],
            &[0x8B,0x45,0xFC,0x50] | &[0x8B,0x4D,0xFC,0x51] | &[0x8B,0x55,0xFC,0x52]
        ){4}else if matches!(&w[5..11],
            &[0x8B,0x4D,0x08,0x8B,0x11,0x52] | &[0x8B,0x45,0x08,0x8B,0x08,0x51]
        ){6}else{continue};
        let call=5+prelude;
        if w[call]!=0xE8{continue}
        let disp=i32::from_le_bytes(w[call+1..call+5].try_into().unwrap());
        let call_rva=site.checked_add(call as u32).ok_or("call overflow")?;
        if i64::from(call_rva)+5+i64::from(disp)!=i64::from(resolver_rva){continue}
        let after=call+5;
        if &w[after..after+3]!=&[0x83,0xC4,0x08]{continue}
        let store=after+3;
        if w[store]!=0xA3{continue}
        let slot_va=u32::from_le_bytes(w[store+1..store+5].try_into().unwrap());
        let cmp=store+5;
        if &w[cmp..cmp+2]!=&[0x83,0x3D] || w[cmp+6]!=0 {continue}
        let other_va=u32::from_le_bytes(w[cmp+2..cmp+6].try_into().unwrap());
        if slot_va!=other_va{continue}
        let Some(output_slot_rva)=slot_va.checked_sub(image_base) else{continue};
        let store_rva=site.checked_add((store+1) as u32).ok_or("store RVA overflow")?;
        let cmp_rva=site.checked_add((cmp+2) as u32).ok_or("cmp RVA overflow")?;
        if !relocations.contains(&store_rva)||!relocations.contains(&cmp_rva){continue}
        hits.push(ResolverSlot{
            label_rva,instruction_rva:site,call_rva,output_slot_rva
        });
    }
    hits.sort_by_key(|r|(r.output_slot_rva,r.instruction_rva));
    Ok(hits)
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
    fn metadata_push_xrefs_match_literal_encoded_x86_va() {
        let labels = vec![Label { rva: 0x20508, value: "Example".into() }];
        let mut code = vec![0x90u8; 64];
        let absolute = 0x0090_0508u32.to_le_bytes();
        code[7] = 0x68;
        code[8..12].copy_from_slice(&absolute);
        code[24] = 0x6A; // Different PUSH opcode, not an immediate DWORD.
        code[25..29].copy_from_slice(&absolute);
        let hits=scan_push_references(&code,0x9000,0x008E_0000,&labels).unwrap();
        assert_eq!(hits,vec![PushReference {
            instruction_rva:0x9007,label_rva:0x20508
        }]);
    }

    #[test]
    fn push_operands_are_correlated_with_highlow_sites_not_opcode_sites() {
        let labels = vec![Label { rva: 0x20508, value: "Metadata".into() }];
        let mut code = vec![0x90u8; 40];
        let pointer = 0x0090_0508u32.to_le_bytes();
        for opcode in [7usize, 24] {
            code[opcode] = 0x68;
            code[opcode + 1..opcode + 5].copy_from_slice(&pointer);
        }
        let refs = scan_push_references(&code, 0x9000, 0x008E_0000, &labels).unwrap();
        assert_eq!(refs.len(), 2);
        // At 0x9008 the relocation is on the immediate DWORD. 0x9018 is
        // the second opcode; that entry must not count as operand evidence.
        let slots = std::collections::HashSet::from([0x9008u32, 0x9018]);
        assert_eq!(push_operand_highlow_flags(&refs, &slots).unwrap(), vec![true, false]);
        let extra = [PushReference { instruction_rva: u32::MAX, label_rva: 1 }];
        assert!(push_operand_highlow_flags(&extra, &slots).is_err());
    }

    #[test]
    fn relocated_label_scan_handles_cross_page_dwords_and_other_encodings() {
        let labels = vec![Label { rva: 0x20508, value: "Label".into() }];
        let mut bytes = vec![0u8; 4112];
        let address = 0x0090_0508u32.to_le_bytes();
        bytes[4..8].copy_from_slice(&address); // Matching but NOT relocated.
        bytes[0xFFE - 1] = 0x68; // Preceding byte only, not disassembly.
        bytes[0xFFE..0x1002].copy_from_slice(&address);
        bytes[0x1005..0x1009].copy_from_slice(&address);
        let slots = std::collections::HashSet::from([
            0x9FFEu32, 0xA005, 0x9000, 0xA300,
        ]);
        let result = scan_relocated_label_pointers(
            &bytes, 0x9000, 0x008E_0000, &labels, &slots,
        ).unwrap();
        assert_eq!(result, vec![
            RelocatedLabelPointer {
                operand_rva: 0x9FFE, label_rva: 0x20508, previous_byte: Some(0x68),
            },
            RelocatedLabelPointer {
                operand_rva: 0xA005, label_rva: 0x20508, previous_byte: Some(0),
            },
        ]);
        assert!(scan_relocated_label_pointers(
            &bytes, 0x9000, u32::MAX, &labels, &slots,
        ).is_err());
    }

    #[test]
    fn api_resolver_slots_require_real_three_operand_highlow_relocations() {
        let base=0x008E_0000u32;
        let mut code=vec![0x90u8;128];
        let label=Label{rva:0x20660,value:"Example".into()};
        let instruction=0x10usize;
        let mut i=instruction;
        code[i]=0x68;
        code[i+1..i+5].copy_from_slice(&(base+label.rva).to_le_bytes());
        i+=5;
        code[i..i+4].copy_from_slice(&[0x8B,0x55,0xFC,0x52]);
        i+=4;
        code[i]=0xE8;
        let disp=(i64::from(0xC708u32)-i64::from(0xD000u32+i as u32+5)) as i32;
        code[i+1..i+5].copy_from_slice(&disp.to_le_bytes());
        i+=5;
        code[i..i+3].copy_from_slice(&[0x83,0xC4,0x08]);
        i+=3;
        code[i]=0xA3;
        code[i+1..i+5].copy_from_slice(&(base+0x24F00u32).to_le_bytes());
        let store=i+1;
        i+=5;
        code[i..i+2].copy_from_slice(&[0x83,0x3D]);
        code[i+2..i+6].copy_from_slice(&(base+0x24F00u32).to_le_bytes());
        code[i+6]=0;
        let cmp=i+2;
        let highlow=std::collections::HashSet::from([
            0xD011u32,0xD000u32+store as u32,0xD000u32+cmp as u32,
        ]);
        let result=scan_resolver_slots(&code,0xD000,base,&[label.clone()],&highlow,0xC708).unwrap();
        assert_eq!(result,vec![ResolverSlot {
            label_rva:0x20660,instruction_rva:0xD010,
            call_rva:0xD019,output_slot_rva:0x24F00
        }]);
        let mut missing=highlow.clone();
        missing.remove(&(0xD000+cmp as u32));
        assert!(scan_resolver_slots(&code,0xD000,base,&[label.clone()],&missing,0xC708).unwrap().is_empty());
        code[cmp+1]^=0x10;
        assert!(scan_resolver_slots(&code,0xD000,base,&[label],&highlow,0xC708).unwrap().is_empty());
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
