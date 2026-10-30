//! Pure, read-only legacy DirectDraw4 presentation-chain classification and
//! aspect-correct viewport planning.  No COM calls, HWND operations,
//! allocation of DirectDraw surfaces, or mode changes happen in this module.
//! A real windowed presentation adapter is NOT implemented yet.

pub const PRIMARY: u32=0x0000_0200;
pub const BACKBUFFER: u32=0x0000_0004;
pub const COMPLEX: u32=0x0000_0008;
pub const FLIP: u32=0x0000_0010;
pub const ZBUFFER: u32=0x0002_0000;
pub const TEXTURE: u32=0x0000_1000;
pub const MIPMAP: u32=0x0040_0000;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Default)]
pub struct Chain {
    pub front_to_back:bool,
    pub back_to_depth:bool,
    pub mipmaps:bool,
}

impl Chain {
    // Never treat attachments as topology proof merely because they have
    // similar flags: require the exact GetAttachedSurface requested caps.
    pub fn observe(&mut self,request:u32,parent:u32,attached:u32)->bool {
        let before=*self;
        if request&BACKBUFFER!=0 &&
            parent&PRIMARY!=0 && parent&FLIP!=0 &&
            attached&BACKBUFFER!=0 && attached&FLIP!=0 {
            self.front_to_back=true;
        }
        if request&ZBUFFER!=0 &&
            parent&BACKBUFFER!=0 && attached&ZBUFFER!=0 {
            self.back_to_depth=true;
        }
        if request&TEXTURE!=0 && request&MIPMAP!=0 &&
            parent&TEXTURE!=0 && parent&MIPMAP!=0 &&
            attached&TEXTURE!=0 && attached&MIPMAP!=0 {
            self.mipmaps=true;
        }
        *self!=before
    }
    pub fn three_d_ready(self)->bool {self.front_to_back&&self.back_to_depth}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Viewport {pub x:u32,pub y:u32,pub width:u32,pub height:u32}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Mode {pub width:u32,pub height:u32,pub bpp:u32}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowedPresentationPlan {
    pub internal:Mode,
    pub desktop_width:u32,
    pub desktop_height:u32,
    pub viewport:Viewport,
    // Externally visible mode must not be changed until a complete
    // primary/backbuffer/3D virtualizing backend exists.
    pub requires_surface_virtualization:bool,
}

// Integer-only aspect fit using 64-bit intermediate arithmetic avoids
// floating point rounding artifacts, even on an old x86 process.
pub fn fit_viewport(src_width:u32,src_height:u32,
                    dest_width:u32,dest_height:u32)->Option<Viewport> {
    if src_width==0||src_height==0||dest_width==0||dest_height==0{return None;}
    let w=src_width as u64;
    let h=src_height as u64;
    let dw=dest_width as u64;
    let dh=dest_height as u64;
    let (view_w,view_h)=if dw*h<=dh*w {
        (dw,(dw*h/w).max(1))
    }else{
        ((dh*w/h).max(1),dh)
    };
    let view_w=u32::try_from(view_w).ok()?;
    let view_h=u32::try_from(view_h).ok()?;
    Some(Viewport {
        x:(dest_width-view_w)/2,
        y:(dest_height-view_h)/2,
        width:view_w,
        height:view_h,
    })
}

// Only produce a preflight plan for modes actually observed in the user's
// Crimson Skies Wine logs. This does NOT enable borderless at runtime.
pub fn preflight(mode:Mode,desktop_width:u32,desktop_height:u32,
                 chain:Chain)->Option<WindowedPresentationPlan> {
    if !chain.three_d_ready() || mode.bpp!=16{return None;}
    if !matches!((mode.width,mode.height),(640,480)|(800,600)){return None;}
    Some(WindowedPresentationPlan {
        internal:mode,
        desktop_width,
        desktop_height,
        viewport:fit_viewport(mode.width,mode.height,desktop_width,desktop_height)?,
        requires_surface_virtualization:true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_crimson_chain_has_distinct_front_back_depth() {
        let mut chain=Chain::default();
        assert!(chain.observe(0x4,0x1000e238,0x1000601c));
        assert!(!chain.three_d_ready());
        assert!(chain.observe(0x20000,0x1000601c,0x10024000));
        assert!(chain.three_d_ready());
        assert!(chain.observe(0x401000,0x00401808,0x00401808));
        assert!(chain.mipmaps);
    }
    #[test]
    fn wrong_or_failed_attachment_cannot_prove_render_chain() {
        let mut c=Chain::default();
        assert!(!c.observe(ZBUFFER,0x1000e238,0x10024000));
        assert!(!c.observe(BACKBUFFER,0x1000e238,0x10024000));
        assert!(!c.three_d_ready());
        assert!(!c.mipmaps);
    }
    #[test]
    fn four_by_three_mode_is_centered_on_wide_desktop(){
        for (w,h) in [(640,480),(800,600)]{
            assert_eq!(fit_viewport(w,h,1920,1080),
                Some(Viewport{x:240,y:0,width:1440,height:1080}));
        }
    }
    #[test]
    fn fit_handles_portrait_and_smaller_screens(){
        assert_eq!(fit_viewport(800,600,1024,768),
            Some(Viewport{x:0,y:0,width:1024,height:768}));
        assert_eq!(fit_viewport(1920,1080,800,1200),
            Some(Viewport{x:0,y:375,width:800,height:450}));
        assert_eq!(fit_viewport(0,480,1920,1080),None);
        assert_eq!(fit_viewport(640,480,0,1080),None);
    }
    #[test]
    fn no_plan_without_complete_zbuffer_or_supported_mode(){
        let mut c=Chain::default();
        c.observe(BACKBUFFER,0x1000e238,0x1000601c);
        assert!(preflight(Mode{width:800,height:600,bpp:16},1920,1080,c).is_none());
        c.observe(ZBUFFER,0x1000601c,0x10024000);
        assert!(preflight(Mode{width:800,height:600,bpp:32},1920,1080,c).is_none());
        assert!(preflight(Mode{width:1920,height:1080,bpp:16},1920,1080,c).is_none());
        let p=preflight(Mode{width:800,height:600,bpp:16},1920,1080,c).unwrap();
        assert_eq!(p.viewport,Viewport{x:240,y:0,width:1440,height:1080});
        assert!(p.requires_surface_virtualization);
    }
}
