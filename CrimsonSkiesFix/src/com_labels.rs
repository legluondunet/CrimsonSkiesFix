//! Non-mutating DirectDraw / Direct3D interface and surface diagnostics.
//! Classification strings and descriptor inspections are copied without
//! changing runtime semantics or hook ABI. Does not allocate DirectDraw objects.
use std::ffi::c_void;
use crate::{Guid,DDSCAPS_PRIMARYSURFACE,DDSCAPS_BACKBUFFER,DDSCAPS_FLIP,DDSCAPS_COMPLEX,presentation};

pub(crate) fn guid_label(iid:*const Guid)->String {
    if iid.is_null(){return "NULL".to_string();}
    let g=unsafe{&*iid};
    format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.d1,g.d2,g.d3,g.d4[0],g.d4[1],g.d4[2],g.d4[3],
        g.d4[4],g.d4[5],g.d4[6],g.d4[7])
}

// IID matching is deliberately exhaustive rather than assuming a vtable
// layout from the major GUID prefix. Definitions come from Wine's ddraw.h.
pub(crate) fn ddraw_iface(iid:*const Guid)->Option<&'static str> {
    let name=guid_label(iid);
    match name.as_str() {
        "b3a6f3e0-2b43-11cf-a2de-00aa00b93356"=>Some("IDirectDraw2"),
        "9c59509a-39bd-11d1-8c4a-00c04fd930c5"=>Some("IDirectDraw4"),
        "15e65ec0-3b9c-11d2-b92f-00609797ea5b"=>Some("IDirectDraw7"),
        _=>None,
    }
}
pub(crate) fn surface_iface(iid:*const Guid)->Option<&'static str> {
    let name=guid_label(iid);
    match name.as_str() {
        "57805885-6eec-11cf-9441-a82303c10e27"=>Some("IDirectDrawSurface2"),
        "da044e00-69b2-11d0-a1d5-00aa00b8dfbb"=>Some("IDirectDrawSurface3"),
        "0b2b8630-ad35-11d0-8ea6-00609797ea5b"=>Some("IDirectDrawSurface4"),
        "06675a80-3b9b-11d2-b92f-00609797ea5b"=>Some("IDirectDrawSurface7"),
        _=>None,
    }
}


pub(crate) fn surface_roles(caps:u32)->String {
    let mut roles=Vec::new();
    if caps&DDSCAPS_PRIMARYSURFACE!=0{roles.push("primary");}
    if caps&DDSCAPS_BACKBUFFER!=0{roles.push("backbuffer");}
    if caps&DDSCAPS_FLIP!=0{roles.push("flip-chain");}
    if caps&DDSCAPS_COMPLEX!=0{roles.push("complex");}
    if caps&presentation::ZBUFFER!=0{roles.push("z-buffer");}
    if caps&presentation::TEXTURE!=0{roles.push("texture");}
    if caps&presentation::MIPMAP!=0{roles.push("mipmap");}
    if roles.is_empty(){"other".into()}else{roles.join("+")}
}

pub(crate) fn describe_surface_layout(desc:*const c_void)->String{
    if desc.is_null(){return "NULL descriptor".into();}
    unsafe{
        // The first 16 bytes are common to DDSURFACEDESC and DDSURFACEDESC2:
        // dwSize, dwFlags, dwHeight and dwWidth. No pixels are inspected.
        let words=desc as *const u32;
        let size=*words;
        if size!=108 && size!=124{return format!("unsupported_descriptor_size={size}");}
        let flags=*words.add(1);
        let h=*words.add(2);
        let w=*words.add(3);
        let dims=if flags&0x6==0x6{
            format!("{w}x{h}")
        }else{"unspecified".into()};
        format!("desc_size={size} flags={flags:#x} dimensions={dims}")
    }
}


pub(crate) fn is_d3d3_iid(iid:*const Guid)->bool {
    guid_label(iid)=="bb223240-e72b-11d0-a9b4-00aa00c0993e"
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_interface_and_caps_are_classified_without_dereference() {
        let null=std::ptr::null();
        assert_eq!(guid_label(null),"NULL");
        assert_eq!(ddraw_iface(null),None);
        assert_eq!(surface_iface(null),None);
        assert!(!is_d3d3_iid(null));
        assert_eq!(surface_roles(0),"other");
        assert_eq!(describe_surface_layout(std::ptr::null()),"NULL descriptor");
    }

    #[test]
    fn known_directdraw_and_surface_interfaces_keep_exact_names() {
        let dd4=Guid{d1:0x9c59509a,d2:0x39bd,d3:0x11d1,d4:[0x8c,0x4a,0,0xc0,0x4f,0xd9,0x30,0xc5]};
        let surf4=Guid{d1:0x0b2b8630,d2:0xad35,d3:0x11d0,d4:[0x8e,0xa6,0,0x60,0x97,0x97,0xea,0x5b]};
        assert_eq!(ddraw_iface(&dd4),Some("IDirectDraw4"));
        assert_eq!(surface_iface(&surf4),Some("IDirectDrawSurface4"));
    }

    #[test]
    fn combined_surface_roles_and_legacy_desc_labels_are_stable() {
        assert_eq!(surface_roles(DDSCAPS_PRIMARYSURFACE|DDSCAPS_FLIP|DDSCAPS_COMPLEX),"primary+flip-chain+complex");
        let desc=[108u32,0x6,600,800];
        assert_eq!(describe_surface_layout(desc.as_ptr().cast()),"desc_size=108 flags=0x6 dimensions=800x600");
        let invalid=[0u32,0,0,0];
        assert_eq!(describe_surface_layout(invalid.as_ptr().cast()),"unsupported_descriptor_size=0");
    }
}
