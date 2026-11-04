//! Canonical build identity, shared by diagnostics and a future verified
//! main-menu renderer. Build SHA is the actual PR head, never the synthetic
//! GitHub pull-request merge commit.
pub fn label()->String{
    format!("CrimsonSkiesFix {}-{}",
        env!("CARGO_PKG_VERSION"),env!("CRIMSONSKIESFIX_BUILD_COMMIT"))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn identity_contains_project_version_and_short_commit(){
        let s=label();
        assert!(s.starts_with(&format!("CrimsonSkiesFix {}-",env!("CARGO_PKG_VERSION"))));
        let commit=s.split('-').next_back().unwrap();
        assert!(!commit.is_empty() && commit.len()<=8);
        assert!(commit=="unknown" || commit.chars().all(|c|c.is_ascii_hexdigit()));
    }
}
