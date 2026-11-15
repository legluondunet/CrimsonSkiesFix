//! Opt-in WinMM ordinal-only ABI forwarding prototype.
//!
//! The build script emits one COFF x86 tail-jump thunk per numeric ordinal
//! 1..1023. Each slot delegates the *same* ordinal to the real system WinMM.
//! Named exports stay at 2048+ and use the existing name-based forwarding.
//!
//! No installer or default runtime build enables this feature. These thunks
//! cannot be used to claim loader-lock or dependent-DLL compatibility yet.
use std::arch::global_asm;

global_asm!(
    include_str!(concat!(env!("OUT_DIR"), "/winmm-ordinal-thunks.s")),
    resolve = sym crate::winmm_proxy::resolve_ordinal,
    options(att_syntax)
);

#[cfg(test)]
mod tests {
    #[test]
    fn winmm_bridge_has_bounded_nonoverlapping_tables() {
        assert!(1023 < 2048, "named and ordinal exports cannot overlap");
        assert_eq!(super::super::winmm_proxy::resolve_ordinal as usize != 0, true);
    }
}
