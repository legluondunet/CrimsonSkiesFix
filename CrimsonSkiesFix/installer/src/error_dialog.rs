//! Native error notification for the consolidated installer.
//! Never modify files, download the updater or change the launch exit code.
//! This is an optional UI after the original diagnostic has been emitted.
use crate::auto_install::EARLIER_CD_UPDATE_PREFIX;

fn should_show(noninteractive:bool,ci:bool,github_actions:bool)->bool{
    !(noninteractive || ci || github_actions)
}

fn update_edition(error:&str)->Option<&str>{
    error.strip_prefix(EARLIER_CD_UPDATE_PREFIX)?
        .lines().next().map(str::trim).filter(|s|!s.is_empty())
}

fn bounded(error:&str)->String{
    const LIMIT:usize=1600;
    let mut chars=error.chars();
    let mut s:String=chars.by_ref().take(LIMIT)
        .map(|c|if c=='\0' {'\u{fffd}'} else {c}).collect();
    if chars.next().is_some(){s.push_str("\n[Further details in the terminal or installer log.]");}
    s
}

fn update_message(edition:&str,folder:&str)->String{
    bounded(&format!("Your original Crimson Skies CD was recognized:\n{edition}\n\n\
        CrimsonSkiesFix requires version 1.02. The official Microsoft 1.02 update \
        is missing, invalid or ambiguous. The game has not started.\n\n\
        1. Download the official update (crmsk102.exe).\n\
        2. If it is an archive, extract the EXE first. Keep exactly one authentic copy \
        beside CrimsonSkiesFix.exe:\n{folder}\n\
        3. Do NOT run the Microsoft updater. Restart CrimsonSkiesFix.exe.\n\n\
        Open the download instructions in your default browser now?"))
}

fn generic_message(error:&str)->String{
    format!("CrimsonSkiesFix could not complete the requested operation.\n\n{}\n\n\
        See CrimsonSkiesFix_installer.log beside the launcher if available. \
        You can also run the launcher from a terminal for the full diagnostic.",
        bounded(error.trim()))
}

#[cfg(windows)]
mod native{
    use super::*;
    use std::{env,ffi::{c_void,OsStr},ptr};

    const MB_ICONERROR:u32=0x10;
    const MB_ICONWARNING:u32=0x30;
    const MB_YESNO:u32=0x04;
    const MB_DEFBUTTON2:u32=0x100;
    const MB_SETFOREGROUND:u32=0x10000;
    const IDYES:i32=6;
    #[link(name="user32")]
    extern "system"{
        fn MessageBoxW(window:*mut c_void,text:*const u16,caption:*const u16,kind:u32)->i32;
    }
    #[link(name="shell32")]
    extern "system"{
        fn ShellExecuteW(window:*mut c_void,operation:*const u16,file:*const u16,
                         parameters:*const u16,directory:*const u16,show:i32)->isize;
    }

    fn wide(s:&str)->Vec<u16>{
        s.replace('\0',"\u{fffd}").encode_utf16().chain(Some(0)).collect()
    }
    fn dialog(title:&str,message:&str,kind:u32)->i32{
        let title=wide(title);
        let message=wide(message);
        unsafe{MessageBoxW(ptr::null_mut(),message.as_ptr(),title.as_ptr(),kind|MB_SETFOREGROUND)}
    }
    fn open_download_instructions(){
        // Never pass paths or error details from untrusted input to the
        // shell. Only open this fixed project HTTPS URL after explicit Yes.
        let operation=wide("open");
        let url=wide(crate::update102::UPDATE_INSTRUCTIONS_URL);
        let result=unsafe{ShellExecuteW(ptr::null_mut(),operation.as_ptr(),url.as_ptr(),
            ptr::null(),ptr::null(),1)};
        if result<=32{
            dialog("CrimsonSkiesFix - Browser unavailable",
                &format!("Could not open your default browser.\n\n\
                    Open this address manually:\n{}",crate::update102::UPDATE_INSTRUCTIONS_URL),
                MB_ICONWARNING);
        }
    }

    pub(super) fn show(error:&str){
        // Report every blocking launcher error in both stderr and a native
        // dialog, regardless of terminal, GUI launch or command-line arguments.
        // Only the explicit unattended flag and CI suppress the modal UI.
        let noninteractive=env::var_os("CRIMSONSKIESFIX_NONINTERACTIVE").as_deref()
            ==Some(OsStr::new("1"));
        if !should_show(noninteractive,env::var_os("CI").is_some(),
            env::var_os("GITHUB_ACTIONS").is_some()){return;}
        if let Some(edition)=update_edition(error){
            let folder=env::current_exe().ok()
                .and_then(|p|p.parent().map(|d|d.display().to_string()))
                .unwrap_or_else(||"the directory containing CrimsonSkiesFix.exe".into());
            if dialog("CrimsonSkiesFix - Official 1.02 update required",
                &update_message(edition,&folder),MB_ICONWARNING|MB_YESNO|MB_DEFBUTTON2)==IDYES{
                open_download_instructions();
            }
        }else{
            dialog("CrimsonSkiesFix - Error",&generic_message(error),MB_ICONERROR);
        }
    }
}

pub(super) fn show_unless_noninteractive(error:&str){
    #[cfg(windows)]
    native::show(error);
    #[cfg(not(windows))]
    let _=error;
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn interactive_errors_always_show_dialog(){
        assert!(should_show(false,false,false));
    }
    #[test]
    fn only_unattended_or_ci_suppress_dialog(){
        for noninteractive in [false,true]{
            for ci in [false,true]{
                for github_actions in [false,true]{
                    assert_eq!(should_show(noninteractive,ci,github_actions),
                        !noninteractive && !ci && !github_actions);
                }
            }
        }
    }
    #[test]
    fn update_dialog_requires_exact_recognized_cd_prefix(){
        let err=format!("{EARLIER_CD_UPDATE_PREFIX} German, original version 1.0 (C_SKIES_DE_001).\nDetails: missing");
        assert_eq!(update_edition(&err),Some("German, original version 1.0 (C_SKIES_DE_001)."));
        assert_eq!(update_edition("missing CD image"),None);
        assert_eq!(update_edition(&format!("generic error:{err}")),None);
        assert_eq!(update_edition(EARLIER_CD_UPDATE_PREFIX),None);
        let msg=update_message(update_edition(&err).unwrap(),r"C:\Games\Crimson");
        for expected in ["German","1.02","crmsk102.exe",r"C:\Games\Crimson",
            "Do NOT run","Restart CrimsonSkiesFix.exe","default browser"]{
            assert!(msg.contains(expected),"{msg}");
        }
    }
    #[test]
    fn generic_diagnostic_has_log_and_safe_bounded_text(){
        let msg=generic_message("missing CD image");
        assert!(msg.contains("missing CD image"));
        assert!(msg.contains("CrimsonSkiesFix_installer.log"));
        let long=bounded(&("é\0".repeat(900)));
        assert!(!long.contains('\0'));
        assert!(long.contains("Further details"));
        assert!(long.chars().count()<1700);
    }
}
