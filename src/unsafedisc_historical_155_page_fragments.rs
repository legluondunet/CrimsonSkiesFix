// Independent read-only historical 4KiB fragment geometry model.
// Never executes SafeDisc code, decodes a DLL or alters game bytes.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

/// Structural audit of fragments sent independently to the historical
/// 0x401F01 helper from a single selected code section. This computes
/// metadata only; it is NOT evidence that the real loader visited
/// those exact code paths or that an opaque DLL modifier is recovered.
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
pub struct PageFragmentAudit {
    pub relocations:usize,
    pub excluded_bytes:usize,
    pub included_bytes:usize,
    pub fragment_calls:usize,
    pub unaligned_calls:usize,
    pub complete_dwords:usize,
    pub unaligned_dwords:usize,
    pub incomplete_tail_bytes:usize,
    pub crossing_relocations:usize,
    pub max_fragment_dwords:usize,
}

/// Iterate explicit (start,end) intervals per 4096-byte page, NOT a
/// byte-level marking bitmap. For each page, an inherited cross-page
/// relocation exclusion is clipped to that page; fragments restart
/// the modifier's DWORD index at 0, and 0..3 trailing bytes are not
/// transformed by the historical extra-DWORD layer.
/// Caller must supply sorted, nonoverlapping, in-section HIGHLOW
/// offset *starts*. Nonzero ABSOLUTE, mid-block padding and other
/// historic control-flow hazards require separate preflight.
pub fn independent_page_fragment_geometry(
    extent:usize,highlow_starts:&[usize],
)->Result<PageFragmentAudit,String>{
    if extent==0 {return Err("empty selected PE section".into());}
    let mut previous_end=0usize;
    let mut windows=Vec::with_capacity(highlow_starts.len());
    for &start in highlow_starts {
        let end=start.checked_add(4).ok_or("HIGHLOW position overflow")?;
        if start<previous_end || end>extent {
            return Err("unsorted, overlapping or out-of-extent HIGHLOW".into());
        }
        windows.push((start,end));
        previous_end=end;
    }
    let excluded=highlow_starts.len().checked_mul(4)
        .ok_or("total relocation exclusion overflow")?;
    let mut report=PageFragmentAudit{
        relocations:highlow_starts.len(),excluded_bytes:excluded,
        ..PageFragmentAudit::default()
    };
    let mut next_window=0usize;
    for page_start in (0..extent).step_by(4096){
        let page_end=page_start.saturating_add(4096).min(extent);
        while next_window<windows.len() && windows[next_window].1<=page_start {
            next_window+=1;
        }
        let mut cursor=page_start;
        let mut i=next_window;
        while i<windows.len() && windows[i].0<page_end {
            let (start,end)=windows[i];
            let gap_end=start.max(page_start);
            if cursor<gap_end {
                record_fragment(&mut report,cursor,gap_end,page_start)?;
            }
            cursor=cursor.max(end.min(page_end));
            if start<page_end && end>page_end{
                report.crossing_relocations+=1;
            }
            i+=1;
        }
        if cursor<page_end {
            record_fragment(&mut report,cursor,page_end,page_start)?;
        }
    }
    if report.included_bytes.checked_add(report.excluded_bytes)!=Some(extent){
        return Err("independent page-fragment accounting mismatch".into());
    }
    Ok(report)
}

fn record_fragment(
    report:&mut PageFragmentAudit,start:usize,end:usize,page_start:usize,
)->Result<(),String>{
    let len=end.checked_sub(start)
        .ok_or("fragment end before start")?;
    report.included_bytes=report.included_bytes.checked_add(len)
        .ok_or("fragment included-byte count overflow")?;
    let dwords=len/4;
    report.incomplete_tail_bytes=report.incomplete_tail_bytes
        .checked_add(len%4).ok_or("fragment tail count overflow")?;
    report.complete_dwords=report.complete_dwords
        .checked_add(dwords).ok_or("fragment DWORD count overflow")?;
    if dwords>0 {
        report.fragment_calls=report.fragment_calls.checked_add(1)
            .ok_or("fragment count overflow")?;
        report.max_fragment_dwords=report.max_fragment_dwords.max(dwords);
        if (start-page_start)%4!=0{
            report.unaligned_calls=report.unaligned_calls.checked_add(1)
                .ok_or("unaligned fragment count overflow")?;
            report.unaligned_dwords=report.unaligned_dwords
                .checked_add(dwords).ok_or("unaligned DWORD count overflow")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_no_relocations_and_odd_extent(){
        assert!(independent_page_fragment_geometry(0,&[]).is_err());
        let r=independent_page_fragment_geometry(4099,&[]).unwrap();
        assert_eq!(r.included_bytes,4099);
        assert_eq!(r.excluded_bytes,0);
        assert_eq!(r.fragment_calls,1);
        assert_eq!(r.complete_dwords,1024);
        assert_eq!(r.incomplete_tail_bytes,3);
        assert_eq!(r.max_fragment_dwords,1024);
    }
    #[test]
    fn contiguous_relocated_dwords_create_no_middle_fragment(){
        let r=independent_page_fragment_geometry(4096,&[0,4]).unwrap();
        assert_eq!(r.relocations,2);
        assert_eq!(r.excluded_bytes,8);
        assert_eq!(r.fragment_calls,1);
        assert_eq!(r.complete_dwords,1022);
        assert_eq!(r.max_fragment_dwords,1022);
        assert_eq!(r.incomplete_tail_bytes,0);
    }
    #[test]
    fn cross_page_highlow_at_fff_excludes_first_three_bytes_of_next_page(){
        let r=independent_page_fragment_geometry(8192,&[4095]).unwrap();
        assert_eq!(r.crossing_relocations,1);
        assert_eq!(r.excluded_bytes,4);
        assert_eq!(r.included_bytes,8188);
        assert_eq!(r.fragment_calls,2);
        assert_eq!(r.complete_dwords,2046);
        assert_eq!(r.incomplete_tail_bytes,4);
        assert_eq!(r.unaligned_calls,1);
        assert_eq!(r.unaligned_dwords,1023);
        assert_eq!(r.max_fragment_dwords,1023);
    }
    #[test]
    fn fragments_restart_word_index_at_every_gap_and_page(){
        let r=independent_page_fragment_geometry(8192,&[8,20,4096,4100]).unwrap();
        assert_eq!(r.fragment_calls,4);
        assert_eq!(r.complete_dwords,2044);
        assert_eq!(r.excluded_bytes,16);
        assert_eq!(r.max_fragment_dwords,1022); // second page: [4104,8192)
        assert_eq!(r.incomplete_tail_bytes,0);
    }
    #[test]
    fn unaligned_gaps_have_dword_tail_bytes_not_global_alignment(){
        let r=independent_page_fragment_geometry(16,&[1,9]).unwrap();
        assert_eq!(r.excluded_bytes,8);
        assert_eq!(r.complete_dwords,1);
        assert_eq!(r.incomplete_tail_bytes,4);
        assert_eq!(r.unaligned_calls,1);
        assert_eq!(r.unaligned_dwords,1);
    }
    // Independent test oracle with one flag per BYTE. Unlike the
    // production interval walker, it never scans (start,end) pairs.
    // It is synthetic only, not the original game's input.
    fn bitmap_reference(extent:usize,slots:&[usize])->PageFragmentAudit{
        let mut excluded=vec![false;extent];
        for &start in slots {excluded[start..start+4].fill(true);}
        let mut out=PageFragmentAudit{
            relocations:slots.len(),
            excluded_bytes:slots.len()*4,
            ..PageFragmentAudit::default()
        };
        for page_start in (0..extent).step_by(4096){
            let stop=extent.min(page_start+4096);
            let mut i=page_start;
            while i<stop{
                if excluded[i] {i+=1;continue;}
                let start=i;
                while i<stop && !excluded[i] {i+=1;}
                let len=i-start;
                out.included_bytes+=len;
                out.incomplete_tail_bytes+=len%4;
                let dw=len/4;
                out.complete_dwords+=dw;
                if dw>0{
                    out.fragment_calls+=1;
                    out.max_fragment_dwords=out.max_fragment_dwords.max(dw);
                    if (start-page_start)%4!=0{
                        out.unaligned_calls+=1;
                        out.unaligned_dwords+=dw;
                    }
                }
            }
        }
        out.crossing_relocations=slots.iter()
            .filter(|&&start|start/4096!=(start+3)/4096).count();
        out
    }

    #[test]
    fn synthetic_boundary_positions_match_independent_bitmap_model(){
        for extent in [4096usize,4097,4100,8192,8197]{
            // Include the legitimate offset-zero slot, 4-byte
            // adjacent pairs and starts crossing each 4KiB edge.
            let starts:Vec<_>=(0..extent).filter(|&at|
                at+4<=extent &&
                (at<=8 || (4090..=4103).contains(&at) ||
                 (8187..=8193).contains(&at))
            ).collect();
            for &at in &starts{
                let list=[at];
                assert_eq!(independent_page_fragment_geometry(extent,&list).unwrap(),
                    bitmap_reference(extent,&list),"extent={extent} at={at}");
            }
            for (i,&a) in starts.iter().enumerate(){
                for &b in &starts[i+1..]{
                    if b<a+4 {continue;}
                    let list=[a,b];
                    assert_eq!(independent_page_fragment_geometry(extent,&list).unwrap(),
                        bitmap_reference(extent,&list),"extent={extent} pair={a},{b}");
                }
            }
        }
    }

    #[test]
    fn synthetic_deterministic_fuzz_matches_byte_bitmap(){
        let mut state=0xA5B1_C7D9u32;
        for trial in 0..240{
            state=state.wrapping_mul(1664525).wrapping_add(1013904223);
            let extent=[5usize,16,31,4095,4096,4097,4100,8192,8193,12288]
                [(state as usize)%10];
            let mut slots=Vec::new();
            let mut cursor=0usize;
            loop{
                state=state.wrapping_mul(1664525).wrapping_add(1013904223);
                cursor=cursor.saturating_add((state as usize)%21);
                if cursor.checked_add(4).is_none_or(|end|end>extent){break;}
                slots.push(cursor);
                cursor+=4;
            }
            let actual=independent_page_fragment_geometry(extent,&slots).unwrap();
            let expected=bitmap_reference(extent,&slots);
            assert_eq!(actual,expected,"synthetic trial {trial}, extent {extent}");
        }
    }

    #[test]
    fn malformed_overlapping_or_out_of_bounds_relocations_fail_closed(){
        for bad in [&[8,4][..],&[3,5][..],&[4094][..]]{
            assert!(independent_page_fragment_geometry(4096,bad).is_err());
        }
        assert!(independent_page_fragment_geometry(4096,&[usize::MAX]).is_err());
        assert!(independent_page_fragment_geometry(4096,&[4095]).is_err());
    }
}
