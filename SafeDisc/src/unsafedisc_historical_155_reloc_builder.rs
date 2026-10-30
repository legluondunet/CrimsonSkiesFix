// Static, read-only reconstruction of the archived unSafeDisc 1.5.5
// PE relocation cutpoint builder. Public compatibility variant only.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155::historical_155_branch_target;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalRelocCutpointAudit {
    pub mode_global_va: u32,
    pub reloc_mode_value: u32,
    pub first_cutpoint_va: u32,
    pub cutpoint_guard_va: u32,
    pub skip_zero_words: bool,
    pub filters_reloc_type: bool,
}

const ARCHIVED_RELOC_WINDOWS: &[(u32,&[u8])] = &[
        // Look for ".rel" in a first-four-byte section-name compare;
        // section-count LOOP (when this fallback path is entered).
        (0x402358,&[
            0x81,0x38,0x2E,0x72,0x65,0x6C,0x74,0x06,
            0x83,0xC0,0x28,0xE2,0xF3,0xC3,
        ]),
        // Enter builder, place 4 before first cutpoint at 0x403CB0,
        // read relocation section file pointer.
        (0x402366,&[
            0x60,0xBF,0xB0,0x3C,0x40,0,
            0xC7,0x47,0xFC,0x04,0,0,0,
            0x8B,0x70,0x14,0x03,0x35,0x0F,0x3B,0x40,0,
            0x89,0x35,0xBC,0xBC,0x60,0,
        ]),
        // relocation block VirtualAddress/SizeOfBlock and page-end
        // cutpoint (VA+0x1FFF)&0x7FFFF000.
        (0x402382,&[
            0x8B,0x1E,0x8B,0x4E,0x04,0x0B,0xDB,
            0x75,0x03,0x61,0xEB,0xD7,
            0x8B,0xC3,0x05,0xFF,0x1F,0,0,
            0x25,0,0xF0,0xFF,0x7F,
            0xE8,0x2B,0,0,0,
        ]),
        // DWORD block-header skip, then 16-bit reloc words.
        // Only a zero *whole WORD* is excluded; no >>12 type test.
        (0x40239F,&[
            0x83,0xC6,0x08,0x83,0xE9,0x08,0xD1,0xE9,
            0x66,0x8B,0x06,0x83,0xC6,0x02,0x66,0x0B,0xC0,
            0x74,0x14,0x25,0xFF,0x0F,0,0,0x03,0xC3,
            0xE8,0x0C,0,0,0,0x83,0xC0,0x04,
            0xE8,0x04,0,0,0,0xE2,0xDF,0xEB,0xB8,
        ]),
        // Sorted insertion using sentinel DWORD=4. Equal values
        // are shifted, not deduplicated.
        (0x4023CA,&[
            0x83,0xC7,0x04,0x57,0x83,0xEF,0x04,
            0x8B,0x57,0xFC,0x3B,0xC2,0x77,0x04,
            0x89,0x17,0xEB,0xF2,0x89,0x07,0x5F,0xC3,
        ]),
];

/// The exact public archival x86 bytes came from the PINNED 18,944-byte
/// compatibility variant (Git blob 825c467b6b403e1c85a5875ede5696dedfe17652).
/// The owner's exact 20,480-byte original was independently audited via
/// SHA-256-gated inspect-historical in GitHub Actions on 2026-10-28.
/// No original binary is embedded in source or test fixtures.
pub fn audit_155_reloc_cutpoint_builder(
    code:&[u8],image_base:u32,code_rva:u32
)->Result<HistoricalRelocCutpointAudit,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("relocation builder base overflow")?;
    if base!=0x401000 {
        return Err("archived relocation builder CODE base mismatch".into());
    }
    for &(va,bytes) in ARCHIVED_RELOC_WINDOWS {
        let at=va.checked_sub(base)
            .ok_or("archived reloc opcode before CODE")? as usize;
        if code.get(at..at.checked_add(bytes.len())
            .ok_or("archived reloc opcode span overflow")?)!=Some(bytes){
            return Err(format!("archived reloc cutpoint opcode mismatch at 0x{va:08X}"));
        }
    }
    for (from,to) in [
        (0x40235Eu32,0x402366u32),
        (0x402389,0x40238E),
        (0x40238C,0x402365),
        (0x40239A,0x4023CA),
        (0x4023B0,0x4023C6),
        (0x4023B9,0x4023CA),
        (0x4023C1,0x4023CA),
        (0x4023C8,0x402382),
        (0x4023D6,0x4023DC),
        (0x4023DA,0x4023CE),
    ] {
        if historical_155_branch_target(code,base,from)?!=to {
            return Err(format!("archived cutpoint branch mismatch at 0x{from:08X}"));
        }
    }
    // LOOP is not supported by the small generic Jcc decoder.
    for (from,to) in [(0x402363u32,0x402358u32),
                     (0x4023C6,0x4023A7)] {
        let at=(from-base) as usize;
        let word=code.get(at..at+2).ok_or("cutpoint LOOP truncated")?;
        if word[0]!=0xE2 ||
            i64::from(from)+2+i64::from(word[1] as i8)!=i64::from(to){
            return Err("archived cutpoint LOOP target mismatch".into());
        }
    }
    Ok(HistoricalRelocCutpointAudit{
        mode_global_va:0x403CAC,
        reloc_mode_value:4,
        first_cutpoint_va:0x403CB0,
        cutpoint_guard_va:0x403CAC,
        skip_zero_words:true,
        filters_reloc_type:false,
    })
}

/// Synthetic-only cutpoint-generation model. The actual archived x86
/// accepts nonzero relocation words WITHOUT checking the type nibble.
/// Therefore a nonzero type-0 word contributes cutpoints here too.
/// Fail-closed strict bounds and block-size checks are native policy,
/// not behaviors established in historical source.
pub fn hypothetical_155_reloc_cutpoints(
    page_rva:u32,size_of_block:u32,entries:&[u16]
)->Result<Vec<u32>,String>{
    // Original 0x4023A7 reads a WORD before 0x4023C6 LOOP, so
    // a zero-entry block would enter with ECX=0 and wrap for 2^32
    // iterations. The safe preview refuses this unsupported case.
    if entries.is_empty() || page_rva==0 || size_of_block<8 ||
       (size_of_block-8)%2!=0 ||
       usize::try_from((size_of_block-8)/2).ok()!=Some(entries.len()){
        return Err("invalid bounded relocation block geometry".into());
    }
    let rounded=page_rva.checked_add(0x1FFF)
        .ok_or("archived rounded page end exceeds u32")? & 0x7FFF_F000;
    if rounded<=4 {
        return Err("historical sorted cutpoint is not above DWORD guard 4".into());
    }
    let mut out=vec![rounded];
    for &word in entries {
        if word==0 {continue;}
        let start=page_rva.checked_add(u32::from(word&0x0FFF))
            .ok_or("relocation start overflow")?;
        let end=start.checked_add(4)
            .ok_or("relocation cutpoint end overflow")?;
        if start<=4 {
            return Err("relocation cutpoint not above insertion guard 4".into());
        }
        out.push(start);
        out.push(end);
    }
    out.sort_unstable(); // duplicates intentionally retained
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_four_static_anchor_and_rel32_calls(){
        let mut code=vec![0u8;0x2000];
        assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
        for &(va,bytes) in ARCHIVED_RELOC_WINDOWS {
            let at=(va-0x401000) as usize;
            code[at..at+bytes.len()].copy_from_slice(bytes);
        }
        let result=audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).unwrap();
        assert_eq!(result.mode_global_va,0x403CAC);
        assert_eq!(result.reloc_mode_value,4);
        assert!(!result.filters_reloc_type);
        for &(va,_) in ARCHIVED_RELOC_WINDOWS {
            let at=(va-0x401000) as usize;
            code[at]^=1;
            assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
            code[at]^=1;
        }
        let at=(0x4023BA-0x401000) as usize;
        code[at]^=1;
        assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
    }
    #[test]
    fn plain_zero_skipped_but_nonzero_type_zero_is_kept(){
        assert_eq!(hypothetical_155_reloc_cutpoints(
            0x1000,14,&[0,0x3000,0x0001]).unwrap(),
            vec![0x1000,0x1001,0x1004,0x1005,0x2000]);
    }
    #[test]
    fn duplicate_cutpoints_are_not_deduplicated(){
        assert_eq!(hypothetical_155_reloc_cutpoints(
            0x1000,12,&[0x3000,0x3000]).unwrap(),
            vec![0x1000,0x1000,0x1004,0x1004,0x2000]);
    }
    #[test]
    fn invalid_block_length_or_overflow_fails_closed(){
        assert!(hypothetical_155_reloc_cutpoints(0x1000,7,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0x1000,8,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0x1000,9,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0x1000,10,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0,8,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(u32::MAX,8,&[]).is_err());
    }
}


//// Exact x86 LOOP iteration count for the archived relocation WORD
/// builder at 0x4023C6, assuming execution enters 0x4023A7 with
/// ECX=word_count. In particular, ECX=0 does NOT mean zero
/// iterations: the first body runs and the loop wraps to 2^32.
/// This models only the counter, never actual memory reads.
pub fn archived_reloc_builder_loop_iterations(word_count:u32)->u64{
    if word_count==0 {1u64<<32} else {u64::from(word_count)}
}

// Independent read-only scan of IMAGE_BASE_RELOCATION entry WORDs.
/// Unlike a HIGHLOW-only parser, this preserves the distinction
/// between a ZERO type-0 padding WORD and a NONZERO type-0 WORD:
/// historical 4023AD tests only the whole WORD for zero.
///
/// The caller must provide the exact bounded relocation directory
/// bytes, not an arbitrary DLL memory address.
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
pub struct HistoricalRelocWordProfile {
    pub blocks:usize,
    pub highlow_words:usize,
    /// HIGHLOW has a nonzero raw WORD but masks to zero in 401DFF.
    /// The historical consumer's zero-offset branch may treat it
    /// differently from the builder and a modern HIGHLOW walker.
    pub highlow_page_start_words:usize,
    /// A zero-offset HIGHLOW after entry zero is not covered by the
    /// 401E16 first-entry special case and is conservatively refused.
    pub highlow_page_start_not_first_words:usize,
    pub zero_padding_words:usize,
    pub nonzero_type_zero_words:usize,
    pub odd_word_blocks:usize,
    pub empty_word_blocks:usize,
    /// Exact-zero WORDs followed by a nonzero WORD in the SAME block.
    /// Historical 401E31..401E52 can leave its block at that zero
    /// although a modern HIGHLOW-only walker still sees later entries.
    pub nonterminal_zero_padding_words:usize,
    /// Nonempty relocation blocks whose every entry is exact zero.
    /// The historical consumer's initial 401E14 zero special case
    /// need not match treating the entire block as free checksum bytes.
    pub zero_only_blocks:usize,
}

pub fn historical_155_reloc_word_profile(
    directory:&[u8],
)->Result<HistoricalRelocWordProfile,String>{
    if directory.is_empty(){
        return Err("empty historical relocation directory".into());
    }
    let mut at=0usize;
    let mut out=HistoricalRelocWordProfile::default();
    let mut prior=None;
    while at<directory.len(){
        let header=directory.get(at..at.checked_add(8)
            .ok_or("relocation header offset overflow")?)
            .ok_or("truncated historical relocation block header")?;
        let page=u32::from_le_bytes(header[..4].try_into().unwrap());
        let len=u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        if page%4096!=0 || len<8 || len%2!=0 ||
           prior.is_some_and(|v| page<=v){
            return Err("invalid historical relocation block geometry/order".into());
        }
        let end=at.checked_add(len).ok_or("relocation block end overflow")?;
        let block=directory.get(at..end)
            .ok_or("historical relocation block exceeds supplied directory")?;
        let words=(len-8)/2;
        if words==0 {out.empty_word_blocks+=1;}
        if words%2!=0 {out.odd_word_blocks+=1;}
        let mut pending_zeros=0usize;
        let mut any_nonzero=false;
        for (index,entry) in block[8..].chunks_exact(2).enumerate(){
            let word=u16::from_le_bytes([entry[0],entry[1]]);
            match word>>12 {
                0 if word==0 => {
                    out.zero_padding_words+=1;
                    pending_zeros+=1;
                },
                0 => out.nonzero_type_zero_words+=1,
                3 => {
                    out.highlow_words+=1;
                    if word & 0x0FFF == 0 {
                        out.highlow_page_start_words+=1;
                        if index!=0 {out.highlow_page_start_not_first_words+=1;}
                    }
                },
                _ => return Err("unknown historical PE relocation type".into()),
            }
            if word!=0 {
                any_nonzero=true;
                out.nonterminal_zero_padding_words+=pending_zeros;
                pending_zeros=0;
            }
        }
        if words>0 && !any_nonzero {out.zero_only_blocks+=1;}
        out.blocks+=1;
        prior=Some(page);
        at=end;
    }
    Ok(out)
}

#[cfg(test)]
mod relocation_word_profile_tests {
    use super::*;

    fn block(page:u32, words:&[u16])->Vec<u8>{
        let mut out=Vec::new();
        out.extend_from_slice(&page.to_le_bytes());
        out.extend_from_slice(&(8u32+(words.len() as u32)*2).to_le_bytes());
        for &word in words {out.extend_from_slice(&word.to_le_bytes());}
        out
    }

    #[test]
    fn type_zero_nonzero_is_not_the_same_as_zero_padding(){
        let mut directory=block(0x1000,&[0,0x0001,0x3002,0]);
        directory.extend_from_slice(&block(0x2000,&[0x3004,0,0x3008]));
        assert_eq!(historical_155_reloc_word_profile(&directory).unwrap(),
            HistoricalRelocWordProfile{
                blocks:2,highlow_words:3,
                highlow_page_start_words:0,
                highlow_page_start_not_first_words:0,zero_padding_words:3,
                nonzero_type_zero_words:1,odd_word_blocks:1,
                empty_word_blocks:0,nonterminal_zero_padding_words:2,
                zero_only_blocks:0,
            });
    }

    #[test]
    fn historical_type_profile_rejects_unsupported_or_truncated_words(){
        assert!(historical_155_reloc_word_profile(&[]).is_err());
        assert!(historical_155_reloc_word_profile(&[0;7]).is_err());
        assert!(historical_155_reloc_word_profile(&block(0x1000,&[0x2001])).is_err());
        assert!(historical_155_reloc_word_profile(&block(0x1001,&[0x3001])).is_err());
        let mut d=block(0x1000,&[0x3001,0]);
        d.extend_from_slice(&block(0x1000,&[0,0]));
        assert!(historical_155_reloc_word_profile(&d).is_err());
        let d=block(0x2000,&[0,0]);
        assert!(historical_155_reloc_word_profile(&d[..d.len()-1]).is_err());
    }
}


/// Necessary preconditions for comparing a HIGHLOW-only checksum
/// hypothesis to the archived, type-agnostic 1.5.5 cutpoint walker.
/// SUCCESS DOES NOT PROVE EQUIVALENCE: historical zero padding,
/// segment ordering, the checksum start state and DLL modifiers still
/// need independent validation. This is a fail-closed preflight only.
pub fn necessary_highlow_only_candidate_preflight(
    profile:HistoricalRelocWordProfile, expected_highlow:usize, expected_padding:usize,
)->Result<(),String>{
    if profile.blocks==0 || profile.highlow_words==0 {
        return Err("historical relocation profile lacks relocation entries".into());
    }
    if profile.highlow_words!=expected_highlow ||
       profile.zero_padding_words.checked_add(profile.nonzero_type_zero_words)
           !=Some(expected_padding){
        return Err("historical relocation word counts differ from independent PE parser".into());
    }
    if profile.nonzero_type_zero_words!=0 {
        return Err(format!(
            "HIGHLOW-only key hypothesis cannot ignore {} nonzero ABSOLUTE words",
            profile.nonzero_type_zero_words));
    }
    // At 401E16 a ZERO first masked offset jumps to 401E28,
    // which increments by four and excludes the page-start DWORD.
    // Thus a first-entry 0x3000 is intentionally handled, not
    // automatically a HIGHLOW-only checksum divergence.
    if profile.highlow_page_start_not_first_words!=0 {
        return Err(format!(
            "HIGHLOW-only checksum model has {} out-of-order/noninitial HIGHLOW page-start WORDs requiring separate legacy control-flow handling",
            profile.highlow_page_start_not_first_words));
    }
    if profile.nonterminal_zero_padding_words!=0 {
        return Err(format!(
            "HIGHLOW-only key hypothesis cannot ignore {} zero padding WORDs before subsequent relocations",
            profile.nonterminal_zero_padding_words));
    }
    if profile.zero_only_blocks!=0 {
        return Err(format!(
            "HIGHLOW-only key hypothesis cannot safely model {} all-zero relocation blocks",
            profile.zero_only_blocks));
    }
    if profile.odd_word_blocks!=0 || profile.empty_word_blocks!=0 {
        return Err(format!(
            "HIGHLOW-only key hypothesis cannot safely model {} odd and {} empty historical relocation blocks",
            profile.odd_word_blocks,profile.empty_word_blocks));
    }
    Ok(())
}

#[cfg(test)]
mod candidate_preflight_tests {
    use super::*;
    fn good()->HistoricalRelocWordProfile{
        HistoricalRelocWordProfile{
            blocks:2,highlow_words:3,
            highlow_page_start_words:0,
            highlow_page_start_not_first_words:0,zero_padding_words:1,
            nonzero_type_zero_words:0,odd_word_blocks:0,
            empty_word_blocks:0,nonterminal_zero_padding_words:0,
            zero_only_blocks:0,
        }
    }
    #[test]
    fn necessary_preflight_accepts_even_blocks_with_exact_zero_padding(){
        assert!(necessary_highlow_only_candidate_preflight(good(),3,1).is_ok());
        assert!(necessary_highlow_only_candidate_preflight(good(),2,1).is_err());
        assert!(necessary_highlow_only_candidate_preflight(good(),3,2).is_err());
    }
    #[test]
    fn necessary_preflight_rejects_nonzero_absolute_even_when_total_matches(){
        let mut p=good();
        p.nonzero_type_zero_words=1;p.zero_padding_words=0;
        assert!(necessary_highlow_only_candidate_preflight(p,3,1).unwrap_err()
            .contains("nonzero ABSOLUTE"));
    }
    #[test]
    fn necessary_preflight_rejects_odd_or_empty_blocks(){
        let mut p=good();
        p.odd_word_blocks=1;
        assert!(necessary_highlow_only_candidate_preflight(p,3,1).is_err());
        p.odd_word_blocks=0;p.empty_word_blocks=1;
        assert!(necessary_highlow_only_candidate_preflight(p,3,1).is_err());
        p.empty_word_blocks=0;p.highlow_words=0;
        assert!(necessary_highlow_only_candidate_preflight(p,0,1).is_err());
    }
    #[test]
    fn empty_block_x86_loop_wraps_but_native_cutpoint_model_rejects(){
        assert_eq!(archived_reloc_builder_loop_iterations(0),1u64<<32);
        assert_eq!(archived_reloc_builder_loop_iterations(1),1);
        assert_eq!(archived_reloc_builder_loop_iterations(100),100);
        assert!(hypothetical_155_reloc_cutpoints(0x2000,8,&[]).is_err());
    }

    #[test]
    fn word_profile_counts_empty_blocks_separately(){
        let mut bytes=Vec::new();
        bytes.extend_from_slice(&0x1000u32.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&0x2000u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(&0x3001u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        let p=historical_155_reloc_word_profile(&bytes).unwrap();
        assert_eq!(p.empty_word_blocks,1);
        assert_eq!(p.odd_word_blocks,0);
        assert_eq!(p.highlow_words,1);
        assert!(necessary_highlow_only_candidate_preflight(p,1,1).is_err());
    }
}


#[cfg(test)]
mod nonterminal_padding_tests {
    use super::*;

    fn synthetic_directory(entries:&[u16])->Vec<u8> {
        let mut data=vec![0u8;8];
        data[0..4].copy_from_slice(&0x1000u32.to_le_bytes());
        data[4..8].copy_from_slice(&(8u32+(entries.len() as u32)*2).to_le_bytes());
        for &w in entries {data.extend_from_slice(&w.to_le_bytes());}
        data
    }
    #[test]
    fn trailing_zero_padding_is_allowed_by_necessary_preflight(){
        let p=historical_155_reloc_word_profile(
            &synthetic_directory(&[0x3001,0x3010,0,0])
        ).unwrap();
        assert_eq!(p.nonterminal_zero_padding_words,0);
        assert_eq!(p.zero_padding_words,2);
        necessary_highlow_only_candidate_preflight(p,2,2).unwrap();
    }
    #[test]
    fn early_zero_is_not_interchangeable_with_highlow_only_traversal(){
        for entries in [
            vec![0,0x3010,0x3020,0],
            vec![0x3001,0,0x3010,0],
            vec![0x3001,0,0,0x3010],
        ] {
            let p=historical_155_reloc_word_profile(
                &synthetic_directory(&entries)).unwrap();
            assert!(p.nonterminal_zero_padding_words>0);
            let err=necessary_highlow_only_candidate_preflight(
                p,p.highlow_words,p.zero_padding_words
            ).unwrap_err();
            assert!(err.contains("zero padding WORDs before subsequent"));
        }
    }
    #[test]
    fn all_zero_block_is_counted_but_has_no_nonterminal_padding(){
        let p=historical_155_reloc_word_profile(
            &synthetic_directory(&[0,0])
        ).unwrap();
        assert_eq!(p.zero_padding_words,2);
        assert_eq!(p.nonterminal_zero_padding_words,0);
        assert_eq!(p.zero_only_blocks,1);
        let mut with_valid=synthetic_directory(&[0,0]);
        with_valid[0..4].copy_from_slice(&0x1000u32.to_le_bytes());
        with_valid.extend_from_slice(&0x2000u32.to_le_bytes());
        with_valid.extend_from_slice(&12u32.to_le_bytes());
        with_valid.extend_from_slice(&0x3010u16.to_le_bytes());
        with_valid.extend_from_slice(&0u16.to_le_bytes());
        let combined=historical_155_reloc_word_profile(&with_valid).unwrap();
        assert_eq!(combined.zero_only_blocks,1);
        assert!(necessary_highlow_only_candidate_preflight(combined,1,3)
            .unwrap_err().contains("all-zero relocation blocks"));
        assert!(necessary_highlow_only_candidate_preflight(p,0,2).is_err());
    }
}


#[cfg(test)]
mod masked_highlow_zero_tests {
    use super::*;
    fn directory(words:&[u16])->Vec<u8>{
        let mut dir=Vec::new();
        dir.extend_from_slice(&0x1000u32.to_le_bytes());
        dir.extend_from_slice(&(8u32+(words.len()*2) as u32).to_le_bytes());
        for &w in words{dir.extend_from_slice(&w.to_le_bytes());}
        dir
    }
    #[test]
    fn actual_highlow_at_page_start_becomes_zero_after_archived_mask(){
        let p=historical_155_reloc_word_profile(
            &directory(&[0x3000,0x3004,0,0])
        ).unwrap();
        assert_eq!(p.highlow_words,2);
        assert_eq!(p.highlow_page_start_words,1);
        assert_eq!(p.zero_padding_words,2);
        assert_eq!(p.nonterminal_zero_padding_words,0);
        assert_eq!(p.highlow_page_start_not_first_words,0);
        necessary_highlow_only_candidate_preflight(p,2,2).unwrap();
    }
    #[test]
    fn page_start_highlow_after_first_word_requires_distinct_analysis(){
        let p=historical_155_reloc_word_profile(
            &directory(&[0x3004,0x3000,0,0])
        ).unwrap();
        assert_eq!(p.highlow_page_start_words,1);
        assert_eq!(p.highlow_page_start_not_first_words,1);
        assert!(necessary_highlow_only_candidate_preflight(p,2,2)
            .unwrap_err().contains("noninitial"));
    }
    #[test]
    fn ordinary_highlow_does_not_trigger_offset_zero_guard(){
        let p=historical_155_reloc_word_profile(
            &directory(&[0x3001,0x3004,0,0])
        ).unwrap();
        assert_eq!(p.highlow_page_start_words,0);
        necessary_highlow_only_candidate_preflight(p,2,2).unwrap();
    }
}
