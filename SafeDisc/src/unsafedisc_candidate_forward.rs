// Synthetic-tested, read-only forward transform over relocation-excluded
// Crimson .text fragments. No proprietary executable or DLL code embedded.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The caller must SHA-256 validate the original ICD, plaintext reference,
// and mapped Win98 modifier snapshot before using these functions.
// This audit never writes an output executable or changes input slices.

use super::unsafedisc_modifier_snapshot::Pair;

#[derive(Debug,Default,Clone,Copy,PartialEq,Eq)]
pub struct ForwardAudit {
    pub fragments:usize,
    pub complete_words:usize,
    pub excluded_relocation_bytes:usize,
    pub checked_bytes:usize,
    pub matching_bytes:usize,
    pub differing_bytes:usize,
}

// Original 1.5.5 TEA block order with 32-bit wraparound; validated by
// round-tripping the pre-existing inverse operation on synthetic data.
fn tea_decrypt_block(mut left:u32,mut right:u32,key:[u32;4])->(u32,u32) {
    let mut sum=0xC6EF_3720u32;
    for _ in 0..32 {
        right=right.wrapping_sub(
            (left.wrapping_shl(4).wrapping_add(key[2]))
            ^(left.wrapping_add(sum))
            ^((left>>5).wrapping_add(key[3])));
        left=left.wrapping_sub(
            (right.wrapping_shl(4).wrapping_add(key[0]))
            ^(right.wrapping_add(sum))
            ^((right>>5).wrapping_add(key[1])));
        sum=sum.wrapping_sub(0x9E37_79B9);
    }
    (left,right)
}

fn read32(data:&[u8],off:usize)->Result<u32,String> {
    let src=data.get(off..off+4).ok_or("fragment dword out of bounds")?;
    Ok(u32::from_le_bytes(src.try_into().map_err(|_|"invalid dword")?))
}

/// Apply modifier F2(F1(word ^ mask(index)) ^ mask(index)), then
/// decrypt TEA blocks in the same *overlapping order* as unSafeDisc.
/// This transform is limited to one non-relocation page-local interval.
pub fn forward_fragment(
    encrypted:&[u8],key:[u32;4],pair:&Pair
)->Result<Vec<u8>,String> {
    let mut result=encrypted.to_vec();
    for (index,slot) in result.chunks_exact_mut(4).enumerate(){
        let input=u32::from_le_bytes(slot.try_into()
            .map_err(|_|"invalid complete fragment word")?);
        let index=u32::try_from(index)
            .map_err(|_|"fragment index exceeds x86 DWORD")?;
        slot.copy_from_slice(&pair.extra_word(input,index).to_le_bytes());
    }
    let n=result.len();
    if n<8 {return Ok(result);}
    let mut cursor=0usize;
    let mut remaining=n;
    while remaining>15 {
        let (left,right)=tea_decrypt_block(
            read32(&result,cursor)?,read32(&result,cursor+4)?,key);
        result[cursor..cursor+4].copy_from_slice(&left.to_le_bytes());
        result[cursor+4..cursor+8].copy_from_slice(&right.to_le_bytes());
        cursor+=8;
        remaining-=8;
    }
    if remaining>=8 {
        // For 9..15 bytes, the original decryptor handles the
        // overlapping last 8-byte block BEFORE the current head.
        if remaining!=8 {
            let at=cursor+remaining-8;
            let (left,right)=tea_decrypt_block(
                read32(&result,at)?,read32(&result,at+4)?,key);
            result[at..at+4].copy_from_slice(&left.to_le_bytes());
            result[at+4..at+8].copy_from_slice(&right.to_le_bytes());
        }
        let (left,right)=tea_decrypt_block(
            read32(&result,cursor)?,read32(&result,cursor+4)?,key);
        result[cursor..cursor+4].copy_from_slice(&left.to_le_bytes());
        result[cursor+4..cursor+8].copy_from_slice(&right.to_le_bytes());
    }
    Ok(result)
}

/// Compare only non-relocated code bytes with an exact private plaintext
/// reference. All private words stay in transient process memory.
/// Relocation DWORDs are not altered: the historical IAT restoration
/// still requires its own separately verified site-specific stage.
pub fn compare_text(
    encrypted:&[u8],plain:&[u8],relocation_slots:&[usize],
    key:[u32;4],pair:&Pair
)->Result<ForwardAudit,String> {
    if encrypted.len()!=plain.len() {
        return Err("ciphertext/plaintext .text span length mismatch".into());
    }
    let mut blocked=vec![false;encrypted.len()];
    let mut previous_end=0usize;
    for &pos in relocation_slots {
        let end=pos.checked_add(4).ok_or("relocation position overflow")?;
        if end>encrypted.len() || pos<previous_end {
            return Err("unsorted, overlapping or unbounded HIGHLOW positions".into());
        }
        blocked[pos..end].fill(true);
        previous_end=end;
    }
    let mut audit=ForwardAudit{
        excluded_relocation_bytes:relocation_slots.len().checked_mul(4)
            .ok_or("relocation byte count overflow")?,
        ..ForwardAudit::default()
    };
    for (page_index,page) in blocked.chunks(4096).enumerate() {
        let page_start=page_index.checked_mul(4096)
            .ok_or("page start overflow")?;
        let mut cursor=0usize;
        while cursor<page.len(){
            if page[cursor] {cursor+=1;continue;}
            let begin=cursor;
            while cursor<page.len() && !page[cursor] {cursor+=1;}
            let first=page_start+begin;
            let last=page_start+cursor;
            let candidate=forward_fragment(&encrypted[first..last],key,pair)?;
            audit.fragments+=usize::from(last-first>=4);
            audit.complete_words+=(last-first)/4;
            for (&x,&y) in candidate.iter().zip(&plain[first..last]){
                audit.checked_bytes+=1;
                audit.matching_bytes+=usize::from(x==y);
                audit.differing_bytes+=usize::from(x!=y);
            }
        }
    }
    if audit.checked_bytes+audit.excluded_relocation_bytes!=encrypted.len()
        || audit.matching_bytes+audit.differing_bytes!=audit.checked_bytes {
        return Err("forward fragment accounting lost bytes".into());
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_pair()->Pair {
        let mut image=vec![0u8;super::super::unsafedisc_modifier_snapshot::END_RVA];
        let a=super::super::unsafedisc_modifier_snapshot::FIRST_RVA;
        let b=super::super::unsafedisc_modifier_snapshot::SECOND_RVA;
        let c=super::super::unsafedisc_modifier_snapshot::END_RVA;
        image[a..b-1].fill(0x40); // 98 x INC EAX
        image[b-1]=0xC3;
        image[b..c-1].fill(0x48); // 98 x DEC EAX
        image[c-1]=0xC3;
        Pair::from_mapped_image(&image).unwrap()
    }

    #[test]
    fn teablock_forward_inverts_existing_reverse_model() {
        let key=[0x7496538F,0x12345678,0x9ABCDEF0,0x0BADF00D];
        for left in [0,1,0x12345678,0xFFFFFFFF] {
            for right in [0,1,0xDEADBEEF,0x7FFFFFFF] {
                let (a,b)=super::super::tea_encrypt_block(left,right,key);
                assert_eq!(tea_decrypt_block(a,b,key),(left,right));
            }
        }
    }

    #[test]
    fn overlapping_tea_and_modifiers_roundtrip_fragments() {
        let pair=synthetic_pair();
        let key=[1,2,3,4];
        for n in 0..65usize {
            let plain=(0..n).map(|i|(i.wrapping_mul(37)^0xAB)as u8)
                .collect::<Vec<_>>();
            let mut intermediate=
                super::super::original_tea_reverse_fragment(&plain,key).unwrap();
            for (index,word) in intermediate.chunks_exact_mut(4).enumerate(){
                let v=u32::from_le_bytes(word.try_into().unwrap());
                word.copy_from_slice(
                    &pair.inverse_extra_word(v,index as u32).to_le_bytes());
            }
            assert_eq!(forward_fragment(&intermediate,key,&pair).unwrap(),plain,
                "wrong forward order for fragment length {n}");
        }
    }

    #[test]
    fn cross_page_highlow_and_fragment_bounds() {
        let pair=synthetic_pair();
        let key=[1,2,3,4];
        let data=vec![0u8;8197];
        let slots=[4093usize,4103,5001,8193];
        let mut reference=data.clone();
        // For zero keys/data the output need not equal the encrypted
        // input, so compare against the independent forward result.
        for (page_index,page) in data.chunks(4096).enumerate() {
            let page_start=page_index*4096;
            let mut cursor=0usize;
            while cursor<page.len(){
                let pos=page_start+cursor;
                if slots.iter().any(|&s|s<=pos&&pos<s+4) {cursor+=1;continue;}
                let begin=cursor;
                while cursor<page.len(){
                    let p=page_start+cursor;
                    if slots.iter().any(|&s|s<=p&&p<s+4) {break;}
                    cursor+=1;
                }
                let first=page_start+begin;
                let last=page_start+cursor;
                reference[first..last].copy_from_slice(
                    &forward_fragment(&data[first..last],key,&pair).unwrap());
            }
        }
        let report=compare_text(&data,&reference,&slots,key,&pair).unwrap();
        assert_eq!(report.differing_bytes,0);
        assert_eq!(report.excluded_relocation_bytes,16);
        assert_eq!(report.checked_bytes,8197-16);
        assert!(compare_text(&data,&reference,&[4095,4093],key,&pair).is_err());
        assert!(compare_text(&data,&reference,&[8195],key,&pair).is_err());
    }
}
