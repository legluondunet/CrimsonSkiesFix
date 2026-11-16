//! Native WinMM read-only XREF audit for the 4:3 HUD aspect investigation.
//! Run ONCE, only when [Debug] Log=1 and the exact game EXE's widescreen
//! byte-gated in-memory patch succeeded. Never patch, intercept or execute
//! candidate instruction locations. These are static absolute-operand
//! candidates, NOT runtime-read proofs.
use super::{get32,GetModuleHandleW,IMAGE_BASE,IMAGE_SIZE};
use std::ptr;

const HUD_ASPECT_GLOBAL:u32=0x0071_B3F8;
const IMAGE_SCN_MEM_EXECUTE:u32=0x2000_0000;
const MAX_REPORTED:usize=40;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Reference {
    instruction_rva:usize,
    kind:&'static str,
}

fn find_references(bytes:&[u8],section_rva:usize,needle:u32)->Vec<Reference>{
    let mut matches=Vec::new();
    let address=needle.to_le_bytes();
    for (offset,operand) in bytes.windows(4).enumerate(){
        if operand!=address {continue;}
        // Classify only a few unambiguous x86 direct-address opcodes.
        // Other occurrences are recorded as 'unknown' because an operand
        // can also be a pointer constant or embedded instruction bytes.
        let (kind,pre)=if offset>=2 {
            match (bytes[offset-2],bytes[offset-1]) {
                (0xD9,0x05)|(0xDB,0x05)|(0xD8,0x05)|
                (0xD8,0x0D)|(0xD8,0x15)|(0xD8,0x25)|
                (0xD8,0x2D)|(0xD8,0x35)|(0xD8,0x3D)|
                (0xDA,0x05)|(0xDA,0x15)|(0xDA,0x25)|
                (0xDA,0x2D)|(0xDA,0x35) => ("x87-read-candidate",2),
                (0xD9,0x15)|(0xD9,0x1D)|(0xDD,0x15)|
                (0xDD,0x1D)|(0xDB,0x15)|(0xDB,0x1D) => ("x87-write-candidate",2),
                (0x8B,m) if m&0xC7==0x05 => ("integer-read-candidate",2),
                (0x89,m) if m&0xC7==0x05 => ("integer-write-candidate",2),
                _ => ("unclassified",0)
            }
        } else {("unclassified",0)};
        let (kind,pre)=if kind=="unclassified" && offset>=1{
            match bytes[offset-1]{
                0xA1=>("integer-read-candidate",1),
                0xA3=>("integer-write-candidate",1),
                _=>(kind,pre)
            }
        }else{(kind,pre)};
        matches.push(Reference {
            instruction_rva:section_rva+offset-pre,kind
        });
    }
    matches
}

// Exact byte context for the three already observed candidate instruction
// starts. This is bounded to the mapped executable section and is emitted
// only under [Debug] Log=1. The bytes are evidence for offline x86 disassembly,
// not a claim that the candidate executed or belongs to HUD rather than FOV.
fn hex_window(bytes:&[u8],section_rva:usize,instruction_rva:usize)
    ->Option<(String,String)>{
    let offset=instruction_rva.checked_sub(section_rva)?;
    if offset>=bytes.len(){return None;}
    let pre=bytes.get(offset.saturating_sub(32)..offset)?;
    let after=bytes.get(offset..offset.saturating_add(64).min(bytes.len()))?;
    let encode=|data:&[u8]|->String{
        let mut s=String::with_capacity(data.len()*2);
        for byte in data {use std::fmt::Write as _;let _=write!(&mut s,"{byte:02x}");}
        s
    };
    Some((encode(pre),encode(after)))
}

fn is_known_aspect_xref(r:&Reference)->bool{
    matches!(
        (r.instruction_rva,r.kind),
        (0x02B58F,"x87-read-candidate")|
        (0x0603FC,"x87-write-candidate")|
        (0x2024C9,"x87-write-candidate")
    )
}

fn read_u16(bytes:&[u8],at:usize)->Option<u16>{
    Some(u16::from_le_bytes(bytes.get(at..at+2)?.try_into().ok()?))
}

pub(super) unsafe fn audit_verified_loaded_exe(){
    let base=GetModuleHandleW(ptr::null());
    if base.is_null()||base as usize!=IMAGE_BASE{
        crate::log("HUD-ASPECT-XREF: skipped unexpected game image base");
        return;
    }
    // The caller has already authenticated the in-memory EXE by checking
    // its PE and EVERY original patch byte before making any edits.
    let image=std::slice::from_raw_parts(base.cast::<u8>(),IMAGE_SIZE);
    let Some(pe)=get32(image,0x3c).map(|v|v as usize)else{return;};
    if image.get(0..2)!=Some(b"MZ") || pe>=0x400 ||
       image.get(pe..pe+4)!=Some(b"PE\0\0") {
        crate::log("HUD-ASPECT-XREF: skipped invalid PE headers");
        return;
    }
    let Some(section_count)=read_u16(image,pe+6).map(usize::from)else{return;};
    let Some(optional_size)=read_u16(image,pe+20).map(usize::from)else{return;};
    if section_count==0||section_count>48{
        crate::log("HUD-ASPECT-XREF: skipped unsupported PE section count");
        return;
    }
    let Some(table)=pe.checked_add(24).and_then(|v|v.checked_add(optional_size))else{return;};
    let mut total=0usize;
    let mut code_sections=0usize;
    for section in 0..section_count{
        let Some(pos)=section.checked_mul(40).and_then(|i|table.checked_add(i))else{return;};
        let Some(record)=image.get(pos..pos+40)else{return;};
        let flags=u32::from_le_bytes(record[36..40].try_into().unwrap());
        if flags&IMAGE_SCN_MEM_EXECUTE==0 {continue;}
        let virtual_size=u32::from_le_bytes(record[8..12].try_into().unwrap()) as usize;
        let virtual_address=u32::from_le_bytes(record[12..16].try_into().unwrap()) as usize;
        let raw_size=u32::from_le_bytes(record[16..20].try_into().unwrap()) as usize;
        let size=virtual_size.max(raw_size);
        if size==0||size>IMAGE_SIZE{continue;}
        let Some(end)=virtual_address.checked_add(size)else{continue;};
        let Some(code)=image.get(virtual_address..end)else{continue;};
        code_sections+=1;
        for r in find_references(code,virtual_address,HUD_ASPECT_GLOBAL){
            total+=1;
            if total<=MAX_REPORTED{
                crate::log(&format!(
                    "HUD-ASPECT-XREF: candidate_rva=0x{:06X} kind={} address=0x{:08X} read_only=true",
                    r.instruction_rva,r.kind,HUD_ASPECT_GLOBAL
                ));
                if is_known_aspect_xref(&r){
                    if let Some((before,after))=
                        hex_window(code,virtual_address,r.instruction_rva){
                        crate::log(&format!(
                            "HUD-ASPECT-CONTEXT: instruction_rva=0x{:06X} before32={} from_instruction64={} interpretation=offline-only",
                            r.instruction_rva,before,after
                        ));
                    }
                }
            }
        }
    }
    crate::log(&format!(
        "HUD-ASPECT-XREF: executable_sections_scanned={code_sections} total_candidates={total} logged={} reference_writer_expected_rva=0x2024C9 analysis_only=true",
        total.min(MAX_REPORTED)
    ));
    // No COM hooks, breakpoints, memory writes or disassembly of unknown
    // instruction boundaries. Further work must correlate candidates with
    // real HUD drawing before changing the shared aspect global.
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn identifies_direct_x87_read_write_and_integer_read_candidates(){
        let text=[
            0x90, 0xD9,0x1D,0xF8,0xB3,0x71,0x00,
            0xD9,0x05,0xF8,0xB3,0x71,0x00,
            0xA1,0xF8,0xB3,0x71,0x00,
            0x90,
        ];
        assert_eq!(find_references(&text,0x2024C8,HUD_ASPECT_GLOBAL),vec![
            Reference{instruction_rva:0x2024C9,kind:"x87-write-candidate"},
            Reference{instruction_rva:0x2024CF,kind:"x87-read-candidate"},
            Reference{instruction_rva:0x2024D5,kind:"integer-read-candidate"}
        ]);
    }
    #[test]
    fn bounded_context_is_only_extracted_from_known_instructions(){
        let data:(Vec<u8>,usize)=((0u8..100).collect(),0x2000);
        let (before,after)=hex_window(&data.0,data.1,0x2010).unwrap();
        assert_eq!(before.len(),16*2);
        assert_eq!(after.len(),64*2);
        assert!(before.starts_with("00010203"));
        assert!(after.starts_with("10111213"));
        assert!(hex_window(&data.0,data.1,0x1fff).is_none());
        assert!(hex_window(&data.0,data.1,0x2064).is_none());
        assert!(is_known_aspect_xref(&Reference{
            instruction_rva:0x02B58F,kind:"x87-read-candidate"
        }));
        assert!(!is_known_aspect_xref(&Reference{
            instruction_rva:0x02B58F,kind:"x87-write-candidate"
        }));
    }
    #[test]
    fn unrelated_floats_and_non_address_operands_do_not_look_like_xrefs(){
        assert!(find_references(&[0xD9,0x05,0x00,0x00,0x80,0x3F],0,HUD_ASPECT_GLOBAL).is_empty());
        let data=[0x90,0xF8,0xB3,0x71,0x00];
        assert_eq!(find_references(&data,0x100,HUD_ASPECT_GLOBAL),
            vec![Reference{instruction_rva:0x101,kind:"unclassified"}]);
    }
}
