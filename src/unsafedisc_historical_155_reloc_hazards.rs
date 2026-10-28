// Read-only, bounded original ICD relocation hazard locator.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155_reloc_builder::{
    HistoricalRelocWordProfile,historical_155_reloc_word_profile,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RelocHazardKind {
    NonzeroAbsoluteWord,
    NoninitialHighlowAtPageStart,
    NonterminalZeroPadding,
    OddWordCount,
    EmptyWordBlock,
    AllZeroNonemptyBlock,
}
impl RelocHazardKind {
    pub fn label(self)->&'static str {
        match self {
            Self::NonzeroAbsoluteWord=>"nonzero type-0 ABSOLUTE word",
            Self::NoninitialHighlowAtPageStart=>"noninitial HIGHLOW at page offset zero (masked to zero)",
            Self::NonterminalZeroPadding=>"zero padding before a later nonzero word",
            Self::OddWordCount=>"odd 16-bit entry count",
            Self::EmptyWordBlock=>"empty relocation block",
            Self::AllZeroNonemptyBlock=>"nonempty block with only zero words",
        }
    }
}
/// Locations are metadata only, not game ciphertext or decrypted code.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct RelocHazardSite {
    pub kind:RelocHazardKind,
    pub block_page_rva:u32,
    pub block_directory_offset:usize,
    pub entry_index:Option<usize>,
    pub raw_word:Option<u16>,
}

/// Revalidate the WHOLE directory with the independently tested
/// WORD profiler before locating hazards. Limit affects output only,
/// never validation. No historical DLL is loaded or executed.
pub fn locate_historical_reloc_hazards(
    directory:&[u8],limit:usize,
)->Result<(HistoricalRelocWordProfile,Vec<RelocHazardSite>),String>{
    let profile=historical_155_reloc_word_profile(directory)?;
    let mut sites=Vec::new();
    let mut at=0usize;
    while at<directory.len(){
        let hdr=directory.get(at..at+8)
            .ok_or("relocation hazard header not backed")?;
        let page=u32::from_le_bytes(hdr[..4].try_into().unwrap());
        let len=u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;
        let end=at.checked_add(len)
            .ok_or("relocation hazard block offset overflow")?;
        let block=directory.get(at+8..end)
            .ok_or("relocation hazard block outside directory")?;
        let words:Vec<u16>=block.chunks_exact(2)
            .map(|b|u16::from_le_bytes([b[0],b[1]])).collect();
        let mut record=|kind,entry_index,raw_word|{
            if sites.len()<limit {
                sites.push(RelocHazardSite{
                    kind,block_page_rva:page,block_directory_offset:at,
                    entry_index,raw_word,
                });
            }
        };
        if words.is_empty(){
            record(RelocHazardKind::EmptyWordBlock,None,None);
        }else{
            if words.len()%2!=0 {
                record(RelocHazardKind::OddWordCount,None,None);
            }
            let last_nonzero=words.iter().rposition(|&w|w!=0);
            if last_nonzero.is_none(){
                record(RelocHazardKind::AllZeroNonemptyBlock,None,None);
            }
            for (index,&word) in words.iter().enumerate(){
                if word==0x3000 && index!=0 {
                    record(RelocHazardKind::NoninitialHighlowAtPageStart,Some(index),Some(word));
                }
                if word!=0 && word>>12==0 {
                    record(RelocHazardKind::NonzeroAbsoluteWord,Some(index),Some(word));
                }
                if word==0 && last_nonzero.is_some_and(|last|index<last){
                    record(RelocHazardKind::NonterminalZeroPadding,Some(index),Some(0));
                }
            }
        }
        at=end;
    }
    Ok((profile,sites))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn block(page:u32,words:&[u16])->Vec<u8>{
        let mut b=Vec::new();
        b.extend_from_slice(&page.to_le_bytes());
        b.extend_from_slice(&(8u32+words.len() as u32*2).to_le_bytes());
        for &w in words {b.extend_from_slice(&w.to_le_bytes());}
        b
    }
    #[test]
    fn reports_exact_page_entry_index_and_kind_without_ciphertext(){
        let mut dir=block(0x1000,&[0x3001,0,0x0004,0x3010]);
        let next_offset=dir.len();
        dir.extend_from_slice(&block(0x2000,&[0,0]));
        let (profile,sites)=locate_historical_reloc_hazards(&dir,100).unwrap();
        assert_eq!(profile.nonterminal_zero_padding_words,1);
        assert_eq!(profile.nonzero_type_zero_words,1);
        assert_eq!(profile.zero_only_blocks,1);
        assert_eq!(sites,vec![
            RelocHazardSite{kind:RelocHazardKind::NonterminalZeroPadding,
                block_page_rva:0x1000,block_directory_offset:0,
                entry_index:Some(1),raw_word:Some(0)},
            RelocHazardSite{kind:RelocHazardKind::NonzeroAbsoluteWord,
                block_page_rva:0x1000,block_directory_offset:0,
                entry_index:Some(2),raw_word:Some(4)},
            RelocHazardSite{kind:RelocHazardKind::AllZeroNonemptyBlock,
                block_page_rva:0x2000,block_directory_offset:next_offset,
                entry_index:None,raw_word:None},
        ]);
    }
    #[test]
    fn rejects_invalid_geometry_and_reports_odd_and_empty_blocks(){
        let mut dir=block(0x1000,&[0x3001]);
        let split=dir.len();
        dir.extend_from_slice(&block(0x2000,&[]));
        let (_,sites)=locate_historical_reloc_hazards(&dir,10).unwrap();
        assert_eq!(sites.len(),2);
        assert_eq!(sites[0].kind,RelocHazardKind::OddWordCount);
        assert_eq!(sites[1].kind,RelocHazardKind::EmptyWordBlock);
        assert_eq!(sites[1].block_directory_offset,split);
        assert!(locate_historical_reloc_hazards(&dir[..dir.len()-1],10).is_err());
    }
    #[test]
    fn return_limit_does_not_bypass_full_validation(){
        let mut dir=block(0x1000,&[0,0x3001,0,0x3002]);
        let (_,sites)=locate_historical_reloc_hazards(&dir,1).unwrap();
        assert_eq!(sites.len(),1);
        assert_eq!(sites[0].kind,RelocHazardKind::NonterminalZeroPadding);
        assert!(locate_historical_reloc_hazards(&dir,0).unwrap().1.is_empty());
        dir.extend_from_slice(&block(0x1000,&[0x3001,0]));
        assert!(locate_historical_reloc_hazards(&dir,0).is_err());
    }
    #[test]
    fn distinguishes_real_highlow_page_start_from_exact_zero_padding(){
        let dir=block(0x1000,&[0x3000,0x3004,0,0]);
        let (profile,sites)=locate_historical_reloc_hazards(&dir,12).unwrap();
        assert_eq!(profile.highlow_page_start_words,1);
        assert_eq!(profile.zero_padding_words,2);
        assert_eq!(profile.highlow_page_start_not_first_words,0);
        // The original 401E16 initial-zero branch explicitly skips
        // a four-byte relocation at page offset zero.
        assert!(sites.is_empty());
    }
    #[test]
    fn noninitial_page_start_relocation_is_reported(){
        let dir=block(0x1000,&[0x3004,0x3000,0,0]);
        let (profile,sites)=locate_historical_reloc_hazards(&dir,12).unwrap();
        assert_eq!(profile.highlow_page_start_not_first_words,1);
        assert_eq!(sites.len(),1);
        assert_eq!(sites[0].kind,RelocHazardKind::NoninitialHighlowAtPageStart);
        assert_eq!(sites[0].entry_index,Some(1));
    }

    #[test]
    fn trailing_padding_is_not_a_nonterminal_hazard(){
        let dir=block(0x1000,&[0x3001,0x3008,0,0]);
        let (profile,sites)=locate_historical_reloc_hazards(&dir,100).unwrap();
        assert_eq!(profile.zero_padding_words,2);
        assert_eq!(profile.nonterminal_zero_padding_words,0);
        assert!(sites.is_empty());
    }
}
