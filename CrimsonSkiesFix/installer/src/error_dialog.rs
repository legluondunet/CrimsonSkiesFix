//! Native error notification for the consolidated installer.
//! Never modify files, download the updater or change the launch exit code.
//! This is an optional UI after the original diagnostic has been emitted.
use crate::auto_install::{EARLIER_CD_UPDATE_PREFIX,CD_MEDIA_ERROR_PREFIX};
use crate::update102::{self,UpdateFailure};

fn should_show(noninteractive:bool,ci:bool,github_actions:bool)->bool{
    !(noninteractive || ci || github_actions)
}

fn update_edition(error:&str)->Option<&'static str>{
    let edition=error.strip_prefix(EARLIER_CD_UPDATE_PREFIX)?.lines().next()?.trim();
    match edition{
        "English (USA/Europe), original version 1.0 (C_SKIES_DISC_001)."=>Some("English (USA/Europe)"),
        "German, original version 1.0 (C_SKIES_DE_001)."=>Some("German"),
        "French, original version 1.0 (C_SKIES_FR_002)."=>Some("French"),
        _=>None,
    }
}

fn update_failure(error:&str)->UpdateFailure{
    error.lines().find_map(|line|line.strip_prefix("Details: "))
        .map(update102::failure_kind).unwrap_or(UpdateFailure::Other)
}

fn cd_message(error:&str)->Option<(&'static str,String)>{
    let detail=error.strip_prefix(CD_MEDIA_ERROR_PREFIX)?;
    let count=detail.lines().next()?.strip_prefix(
        "expected exactly one ISO or CUE beside CrimsonSkiesFix.exe; found ")
        .and_then(|s|s.parse::<usize>().ok());
    let (title,reason)=match count{
        Some(0)=>("CrimsonSkiesFix - Missing game CD","No original Crimson Skies CD image was found."),
        Some(n) if n>1=>("CrimsonSkiesFix - Multiple CDs","More than one original CD image was found."),
        _=>("CrimsonSkiesFix - CD image error","The original CD image could not be opened."),
    };
    Some((title,format!("{reason}\n\n\
        Place one supported CD image (.iso or .cue with its .bin/.img) beside CrimsonSkiesFix.exe.\n\n\
        Restart CrimsonSkiesFix.exe.")))
}

fn bounded(error:&str)->String{
    const LIMIT:usize=1600;
    let mut chars=error.chars();
    let mut s:String=chars.by_ref().take(LIMIT)
        .map(|c|if c=='\0' {'\u{fffd}'} else {c}).collect();
    if chars.next().is_some(){s.push_str("\n[Further details in the terminal or installer log.]");}
    s
}

fn update_message(edition:&str,failure:UpdateFailure)->String{
    let heading=format!("Crimson Skies 1.0 ({edition}) detected.\n\n\
        The official 1.02 update is required to play.");
    if failure==UpdateFailure::Multiple{
        return format!("{heading}\n\nMore than one verified update file was found.\n\
            Keep only one copy beside CrimsonSkiesFix.exe, then restart CrimsonSkiesFix.exe.");
    }
    let reason=match failure{
        UpdateFailure::Missing=>"The update file was not found.",
        UpdateFailure::Invalid=>"An update file failed verification.",
        UpdateFailure::Other=>"The update could not be verified.",
        UpdateFailure::Multiple=>unreachable!(),
    };
    let download=if failure==UpdateFailure::Invalid{
        "Replace the invalid file with the official crmsk102.exe."
    }else{"Download crmsk102.exe."};
    format!("{heading}\n{reason}\n\n\
        1. {download}\n\
        2. Extract it if necessary and place it beside CrimsonSkiesFix.exe.\n\
        3. Restart CrimsonSkiesFix.exe.\n\n\
        Open the download page now?")
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
            let failure=update_failure(error);
            let message=update_message(edition,failure);
            if failure==UpdateFailure::Multiple{
                dialog("CrimsonSkiesFix - Update required",&message,MB_ICONWARNING);
            }else if dialog("CrimsonSkiesFix - Update required",
                &message,MB_ICONWARNING|MB_YESNO|MB_DEFBUTTON2)==IDYES{
                open_download_instructions();
            }
        }else if let Some((title,message))=cd_message(error){
            dialog(title,&message,MB_ICONERROR);
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
    fn update_dialog_uses_short_labels_and_cause_specific_text(){
        for (original,short) in [
            ("German, original version 1.0 (C_SKIES_DE_001).","German"),
            ("French, original version 1.0 (C_SKIES_FR_002).","French"),
            ("English (USA/Europe), original version 1.0 (C_SKIES_DISC_001).","English (USA/Europe)"),
        ]{
            let err=format!("{EARLIER_CD_UPDATE_PREFIX} {original}\nDetails: no exact verified Crimson Skies 1.02 updater found beside launcher");
            assert_eq!(update_edition(&err),Some(short));
            assert_eq!(update_failure(&err),UpdateFailure::Missing);
            let msg=update_message(short,update_failure(&err));
            for expected in [short,"1.02","crmsk102.exe","not found","Restart CrimsonSkiesFix.exe","Open the download page now?"]{
                assert!(msg.contains(expected),"{msg}");
            }
            assert!(!msg.contains("C_SKIES_"));
            assert!(!msg.contains("Do NOT run"));
        }
        assert_eq!(update_edition("missing CD image"),None);
        assert_eq!(update_edition(EARLIER_CD_UPDATE_PREFIX),None);
        let invalid=update_message("German",UpdateFailure::Invalid);
        assert!(invalid.contains("failed verification"));
        assert!(invalid.contains("Replace the invalid file"));
        let duplicate=update_message("German",UpdateFailure::Multiple);
        assert!(duplicate.contains("Keep only one copy"));
        assert!(!duplicate.contains("Download"));
        assert!(!duplicate.contains("Open the download page"));
        assert_eq!(update_failure("Details: found 2 verified 1.02 updater copies; keep exactly one beside the launcher"),UpdateFailure::Multiple);
        assert_eq!(update_failure("Details: no exact verified Crimson Skies 1.02 updater; a candidate failed verification: bad size"),UpdateFailure::Invalid);
        assert_eq!(update_failure("Details: permission denied"),UpdateFailure::Other);
    }
    #[test]
    fn cd_dialog_distinguishes_missing_multiple_and_invalid_media(){
        for (detail,title,reason) in [
            ("expected exactly one ISO or CUE beside CrimsonSkiesFix.exe; found 0","CrimsonSkiesFix - Missing game CD","No original"),
            ("expected exactly one ISO or CUE beside CrimsonSkiesFix.exe; found 2","CrimsonSkiesFix - Multiple CDs","More than one"),
            ("not a regular file: bad.iso","CrimsonSkiesFix - CD image error","could not be opened"),
        ]{
            let err=format!("{CD_MEDIA_ERROR_PREFIX}{detail}\nPlace exactly one supported CD image beside CrimsonSkiesFix.exe.");
            let (actual,msg)=cd_message(&err).unwrap();
            assert_eq!(actual,title);
            assert!(msg.contains(reason),"{msg}");
            assert!(msg.contains("Restart CrimsonSkiesFix.exe"));
            assert!(!msg.contains(detail));
        }
        assert!(cd_message("unrelated error").is_none());
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
