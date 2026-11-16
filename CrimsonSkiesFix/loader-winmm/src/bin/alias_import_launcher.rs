//! TEST-ONLY architecture probe: modify only the suspended child's memory.
//! Native WINMM.dll remains wholly OS-owned (including numeric ordinals).
#![cfg(all(windows,target_arch="x86"))]
use std::{env,ffi::{c_void,OsStr},fs,mem::{size_of,zeroed},os::windows::ffi::OsStrExt,path::PathBuf,ptr};
#[repr(C)]
struct StartupInfoW{
 cb:u32,reserved:*mut u16,desktop:*mut u16,title:*mut u16,
 x:u32,y:u32,width:u32,height:u32,xchars:u32,ychars:u32,
 fill:u32,flags:u32,show:u16,reserved2:u16,reserved_ptr:*mut u8,
 stdin:*mut c_void,stdout:*mut c_void,stderr:*mut c_void,
}
#[repr(C)]
struct ProcessInfo { process:*mut c_void,thread:*mut c_void,pid:u32,tid:u32 }
#[repr(C)]
struct ProcessBasicInfo { reserved:*mut c_void,peb:*mut u8,reserved2:[*mut c_void;2],pid:usize,parent_pid:usize }
#[link(name="kernel32")]
extern "system"{
 fn CreateProcessW(app:*const u16,cmd:*mut u16,pa:*const c_void,ta:*const c_void,
 inherit:i32,flags:u32,env:*const c_void,cwd:*const u16,
 startup:*mut StartupInfoW,info:*mut ProcessInfo)->i32;
 fn ReadProcessMemory(p:*mut c_void,addr:*const c_void,dst:*mut c_void,size:usize,count:*mut usize)->i32;
 fn WriteProcessMemory(p:*mut c_void,addr:*mut c_void,src:*const c_void,size:usize,count:*mut usize)->i32;
 fn VirtualProtectEx(p:*mut c_void,addr:*mut c_void,size:usize,new:u32,old:*mut u32)->i32;
 fn ResumeThread(t:*mut c_void)->u32;
 fn WaitForSingleObject(h:*mut c_void,timeout:u32)->u32;
 fn GetExitCodeProcess(h:*mut c_void,code:*mut u32)->i32;
 fn CloseHandle(h:*mut c_void)->i32;
 fn TerminateProcess(p:*mut c_void,code:u32)->i32;
 fn GetLastError()->u32;
}
#[link(name="ntdll")]
extern "system" {fn NtQueryInformationProcess(p:*mut c_void,class:u32,info:*mut c_void,len:u32,ret:*mut u32)->i32;}
fn wide(s:&OsStr)->Vec<u16>{s.encode_wide().chain(Some(0)).collect()}
fn u16_at(d:&[u8],o:usize)->Result<u16,String>{Ok(u16::from_le_bytes(d.get(o..o+2).ok_or("bad u16")?.try_into().unwrap()))}
fn u32_at(d:&[u8],o:usize)->Result<u32,String>{Ok(u32::from_le_bytes(d.get(o..o+4).ok_or("bad u32")?.try_into().unwrap()))}
fn ddraw_name_rva(data:&[u8])->Result<u32,String>{
 if data.get(0..2)!=Some(b"MZ".as_slice()){return Err("missing MZ".into())}
 let pe=u32_at(data,0x3c)? as usize;
 if data.get(pe..pe+4)!=Some(b"PE\0\0".as_slice()) || u16_at(data,pe+4)?!=0x14c{return Err("not PE32 i386".into())}
 let n=u16_at(data,pe+6)? as usize;
 let optional=pe+24; let size=u16_at(data,pe+20)? as usize;
 if n==0||n>96||size<112||u16_at(data,optional)?!=0x10b{return Err("bad optional header".into())}
 let dir=optional+96+8; let rva=u32_at(data,dir)?; let bytes=u32_at(data,dir+4)?;
 if rva==0||bytes<40||bytes>64*1024{return Err("no bounded import descriptor directory".into())}
 let sections=(0..n).map(|i|{
   let o=optional+size+40*i;
   Ok((u32_at(data,o+12)?,u32_at(data,o+16)?,u32_at(data,o+20)?))
 }).collect::<Result<Vec<_>,String>>()?;
 let offset=|a:u32,len:usize|->Result<usize,String>{
   for (start,raw,ofs) in &sections {
     if a>=*start && (a as u64+len as u64)<=(*start as u64+*raw as u64){
       let p=*ofs as usize + (a-*start) as usize;
       if p.checked_add(len).is_some_and(|end|end<=data.len()){return Ok(p)}
     }
   }
   if a as usize+len<=u32_at(data,optional+60)? as usize && (a as usize+len)<=data.len(){return Ok(a as usize)}
   Err(format!("unmapped RVA {a:#x}"))
 };
 let mut found=None;
 for i in 0..bytes as usize/20 {
    let p=offset(rva+i as u32*20,20)?;
    if data[p..p+20].iter().all(|b|*b==0){break}
    let name_rva=u32_at(data,p+12)?;
    let name_p=offset(name_rva,10)?;
    let name_bytes=&data[name_p..name_p+10];
    if name_bytes.eq_ignore_ascii_case(b"DDRAW.dll\0"){
       if found.replace(name_rva).is_some(){return Err("duplicate DDRAW import".into())}
    }
 }
 found.ok_or_else(||"missing DDRAW.dll import".to_string())
}
struct ChildGuard(ProcessInfo);
impl Drop for ChildGuard{fn drop(&mut self){unsafe{CloseHandle(self.0.thread);CloseHandle(self.0.process);}}}
pub(crate) fn launch_child(target:&std::path::Path,timeout_ms:u32)->Result<(u32,String),String>{
 if !target.is_absolute(){return Err("target must be absolute".into())}
 let name_rva=ddraw_name_rva(&fs::read(target).map_err(|e|e.to_string())?)?;
 let w=wide(target.as_os_str());
 let cwd=wide(target.parent().ok_or("missing parent directory")?.as_os_str());
 let mut si:StartupInfoW=unsafe{zeroed()};si.cb=size_of::<StartupInfoW>() as u32;
 let mut pi:ProcessInfo=unsafe{zeroed()};
 if unsafe{CreateProcessW(w.as_ptr(),ptr::null_mut(),ptr::null(),ptr::null(),0,4,
        ptr::null(),cwd.as_ptr(),&mut si,&mut pi)}==0 {
    return Err(format!("CreateProcessW failed: {}",unsafe{GetLastError()}))
 }
 let child=ChildGuard(pi);
 let failure=||unsafe{GetLastError()};
 let result=(||->Result<(u32,String),String>{
   let mut basic:ProcessBasicInfo=unsafe{zeroed()};let mut length=0;
   let status=unsafe{NtQueryInformationProcess(child.0.process,0,(&mut basic as *mut ProcessBasicInfo).cast(),
      size_of::<ProcessBasicInfo>() as u32,&mut length)};
   if status<0||basic.peb.is_null(){return Err(format!("cannot find remote image base: NTSTATUS={status:#x}"))}
   let mut remote_base=0u32;let mut read=0usize;
   let addr=unsafe{basic.peb.add(8)};
   if unsafe{ReadProcessMemory(child.0.process,addr.cast(),(&mut remote_base as *mut u32).cast(),4,&mut read)}==0||read!=4{
     return Err("could not read PEB.ImageBaseAddress".into())
   }
   let remote_addr=(remote_base as usize).checked_add(name_rva as usize).ok_or("image base overflow")? as *mut c_void;
   let mut name=[0u8;10];read=0;
   if unsafe{ReadProcessMemory(child.0.process,remote_addr,name.as_mut_ptr().cast(),10,&mut read)}==0 || read!=10 || !name.eq_ignore_ascii_case(b"DDRAW.dll\0"){
      return Err(format!("remote preimage mismatch: {name:?}; bytes={read}"))
   }
   let mut old_prot=0u32;
   if unsafe{VirtualProtectEx(child.0.process,remote_addr,10,4,&mut old_prot)}==0{
      return Err(format!("VirtualProtectEx failed: {}",failure()))
   }
   const NEW:&[u8;10]=b"CSFIX.dll\0";
   let mut written=0usize;
   let wrote=unsafe{WriteProcessMemory(child.0.process,remote_addr,NEW.as_ptr().cast(),10,&mut written)}!=0 && written==10;
   let mut discard=0u32;
   let restored=unsafe{VirtualProtectEx(child.0.process,remote_addr,10,old_prot,&mut discard)}!=0;
   if !wrote||!restored{return Err(format!("remote alias write/restore failed wrote={wrote} restored={restored}"))}
   name=[0;10];read=0;
   if unsafe{ReadProcessMemory(child.0.process,remote_addr,name.as_mut_ptr().cast(),10,&mut read)}==0||name!=*NEW{
      return Err("alias patch not visible in child".into())
   }
   println!("ALIAS-IMPORT: patched remote PE32 at RVA {name_rva:#x} ({remote_base:#x}); EXE on disk untouched");
   let resumed=unsafe{ResumeThread(child.0.thread)};
   if resumed==u32::MAX {return Err(format!("ResumeThread failed: {}",failure()))}
   let wait=unsafe{WaitForSingleObject(child.0.process,timeout_ms)};
   if wait!=0{return Err(format!("child timeout/status={wait:#x}"))}
   let mut exit=1u32;
   if unsafe{GetExitCodeProcess(child.0.process,&mut exit)}==0{return Err("cannot get child exit code".into())}
   Ok((exit,format!(
     "ALIAS-LAUNCH: status=patched-resumed child_exit={exit} remote_name_rva={name_rva:#x} native_winmm_imports_unchanged=true on_disk_exe_unchanged=true")))
 })();
 if result.is_err(){unsafe{TerminateProcess(child.0.process,99);}}
 result
}
fn run()->Result<u32,String>{
 let args=env::args_os().skip(1).collect::<Vec<_>>();
 if args.len()!=1{return Err("usage: alias_import_launcher.exe ABSOLUTE_PE32_EXE".into())}
 let (code,_) = launch_child(&PathBuf::from(&args[0]),30000)?;
 Ok(code)
}
fn main(){match run(){Ok(code)=>{println!("ALIAS-IMPORT-LAUNCHER child_exit={code}");std::process::exit(code as i32)},Err(e)=>{eprintln!("ALIAS-IMPORT-LAUNCHER ERROR: {e}");std::process::exit(1)}}}
