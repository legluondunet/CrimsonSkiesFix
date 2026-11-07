//! Read-only, bounded PE32 import audit for the owner-verified game executable.
//! Reports whether the game directly imports a Windows display-mode API.
//! Static imports cannot prove a function was CALLED; dynamically loaded
//! graphics modules and GetProcAddress calls are deliberately out of scope.
//! No COM, process memory, IAT, DPI or monitor state is changed.
use std::fs;

#[derive(Debug, Default, PartialEq, Eq)]
struct Imports {
    mode_apis:Vec<String>,
    directdraw_apis:Vec<String>,
    dynamic_resolver:bool,
}
fn u16_at(data:&[u8],at:usize)->Option<u16>{
    Some(u16::from_le_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn u32_at(data:&[u8],at:usize)->Option<u32>{
    Some(u32::from_le_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn ascii_name(data:&[u8],offset:usize,max:usize)->Option<&str>{
    let limit=offset.checked_add(max)?.min(data.len());
    let b=data.get(offset..limit)?;
    let end=b.iter().position(|x|*x==0)?;
    let bytes=b.get(..end)?;
    if bytes.is_empty() || !bytes.iter().all(|x|x.is_ascii_graphic()){return None;}
    std::str::from_utf8(bytes).ok()
}
struct Image<'a> {
    bytes:&'a [u8],
    sections:Vec<(u32,u32,u32,u32)>, // virtual address, virtual size, raw pointer, raw size
    header_size:u32,
}
impl Image<'_>{
    fn file_offset(&self,rva:u32)->Option<usize>{
        if rva<self.header_size && (rva as usize)<self.bytes.len(){
            return Some(rva as usize);
        }
        for &(va,virtual_size,raw_offset,raw_size) in &self.sections {
            let delta=rva.checked_sub(va)?;
            if delta<virtual_size.max(raw_size) && delta<raw_size {
                let file=raw_offset.checked_add(delta)? as usize;
                if file<self.bytes.len(){return Some(file);}
            }
        }
        None
    }
}
fn inspect(data:&[u8])->Option<Imports>{
    if u16_at(data,0)?!=0x5a4d{return None;}
    let pe=u32_at(data,0x3c)? as usize;
    if u32_at(data,pe)?!=0x0000_4550{return None;}
    let sections=u16_at(data,pe.checked_add(6)?)? as usize;
    let optional_size=u16_at(data,pe.checked_add(20)?)? as usize;
    if sections==0 || sections>96{return None;}
    let opt=pe.checked_add(24)?;
    let opt_end=opt.checked_add(optional_size)?;
    if opt_end>data.len() || u16_at(data,opt)?!=0x10b{return None;} // PE32 x86 only
    let dd_entry=opt.checked_add(104)?; // import directory: PE32 data directory #1
    if dd_entry.checked_add(8)?>opt_end{return None;}
    let imports_rva=u32_at(data,dd_entry)?;
    let header_size=u32_at(data,opt.checked_add(60)?)?;
    if imports_rva==0{return Some(Imports::default());}
    let section_table=opt_end;
    let mut layout=Vec::with_capacity(sections);
    for i in 0..sections {
        let header=section_table.checked_add(i.checked_mul(40)?)?;
        if header.checked_add(40)?>data.len(){return None;}
        layout.push((
            u32_at(data,header+12)?,u32_at(data,header+8)?,
            u32_at(data,header+20)?,u32_at(data,header+16)?
        ));
    }
    let image=Image{bytes:data,sections:layout,header_size};
    let mut found=Imports::default();
    // Bound traversal even if corrupt import-descriptor/thunk terminators.
    for i in 0u32..128 {
        let desc=image.file_offset(imports_rva.checked_add(i.checked_mul(20)?)?)?;
        let oft=u32_at(data,desc)?;
        let name_rva=u32_at(data,desc+12)?;
        let ft=u32_at(data,desc+16)?;
        if oft==0 && name_rva==0 && ft==0 {break;}
        let module=ascii_name(data,image.file_offset(name_rva)?,65)?;
        let module=module.to_ascii_lowercase();
        let thunk_rva=if oft!=0{oft}else{ft};
        if thunk_rva==0{return None;}
        for n in 0u32..4096 {
            let off=image.file_offset(thunk_rva.checked_add(n.checked_mul(4)?)?)?;
            let thunk=u32_at(data,off)?;
            if thunk==0{break;}
            if thunk&0x8000_0000!=0 {continue;} // import by ordinal
            let hint_name=image.file_offset(thunk)?.checked_add(2)?;
            let Some(name)=ascii_name(data,hint_name,96) else{continue;};
            if module=="kernel32.dll" && name=="GetProcAddress"{
                found.dynamic_resolver=true;
            }
            if (module=="user32.dll" && name.starts_with("ChangeDisplaySettings")) ||
                (module=="user32.dll" && name=="SetDisplayConfig") ||
                (module=="win32u.dll" && name.starts_with("NtUserChangeDisplaySettings"))
            {
                if found.mode_apis.len()<24 {
                    found.mode_apis.push(format!("{module}!{name}"));
                }
            }
            if module=="ddraw.dll" && (name.starts_with("DirectDrawCreate") ||
                                      name.starts_with("DirectDrawEnumerate")){
                if found.directdraw_apis.len()<24{
                    found.directdraw_apis.push(name.to_string());
                }
            }
        }
    }
    found.mode_apis.sort();found.mode_apis.dedup();
    found.directdraw_apis.sort();found.directdraw_apis.dedup();
    Some(found)
}
pub fn report(){
    if !crate::diagnostics::is_native_windows() || !crate::cfg().log_enabled{return;}
    // The caller's strict EXE SHA-256 gate has already accepted this exact
    // on-disk game binary; no access to arbitrary process memory is needed.
    let read=std::env::current_exe().ok().and_then(|p|fs::read(p).ok());
    let out=read.as_deref().and_then(inspect);
    match out {
        Some(Imports{mode_apis,directdraw_apis,dynamic_resolver})=>{
            crate::log(&format!(
                "NATIVE-IMPORT-AUDIT: EXE direct_win32_mode_imports={mode_apis:?} directdraw_imports={directdraw_apis:?} uses_GetProcAddress={dynamic_resolver}; STATIC IMPORTS ONLY, no function calls traced"
            ));
        }
        None=>crate::log("NATIVE-IMPORT-AUDIT: original EXE PE32 import directory unavailable; nonfatal"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_or_invalid_import_tables(){
        assert!(inspect(&[]).is_none());
        assert!(inspect(&[0u8;1024]).is_none());
    }
    #[test]
    fn scans_an_import_without_touching_image_bytes(){
        // Tiny synthetic PE32 with a single .idata section at RVA 0x2000,
        // raw file 0x200. It is not executable, just a bounded PE fixture.
        let mut x=vec![0u8;0x600];
        fn write16(x:&mut [u8],i:usize,v:u16){x[i..i+2].copy_from_slice(&v.to_le_bytes());}
        fn write32(x:&mut [u8],i:usize,v:u32){x[i..i+4].copy_from_slice(&v.to_le_bytes());}
        write16(&mut x,0,0x5a4d);write32(&mut x,0x3c,0x80);
        write32(&mut x,0x80,0x4550);
        write16(&mut x,0x86,1);write16(&mut x,0x94,0xe0);
        let opt=0x98;
        write16(&mut x,opt,0x10b);
        write32(&mut x,opt+60,0x200); // SizeOfHeaders
        write32(&mut x,opt+104,0x2000); // Import directory RVA
        let sec=opt+0xe0;
        write32(&mut x,sec+8,0x400);write32(&mut x,sec+12,0x2000);
        write32(&mut x,sec+16,0x400);write32(&mut x,sec+20,0x200);
        write32(&mut x,0x200,0x2040); // OriginalFirstThunk
        write32(&mut x,0x20c,0x2060); // DLL name RVA
        write32(&mut x,0x210,0x2040); // FirstThunk
        write32(&mut x,0x240,0x2080); // IMAGE_IMPORT_BY_NAME RVA
        x[0x260..0x26b].copy_from_slice(b"user32.dll\0");
        let name=b"ChangeDisplaySettingsExW\0";
        x[0x282..0x282+name.len()].copy_from_slice(name);
        let original=x.clone();
        let imports=inspect(&x).expect("synthetic image");
        assert_eq!(imports.mode_apis,vec!["user32.dll!ChangeDisplaySettingsExW"]);
        assert_eq!(x,original);
    }
}
