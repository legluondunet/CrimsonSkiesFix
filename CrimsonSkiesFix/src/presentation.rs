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

// Successive SetDisplayMode calls invalidate the previous COM surface chain.
// Never treat backbuffer/Z-buffer evidence from a *previous* display mode
// as proof that a new mode's attachments have already been established.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Session {
    pub epoch:u64,
    pub mode:Option<Mode>,
    pub chain:Chain,
    pub preflight_reported:bool,
}

impl Session {
    pub const fn new()->Self {
        Self {epoch:0,mode:None,
              chain:Chain{front_to_back:false,back_to_depth:false,mipmaps:false},
              preflight_reported:false}
    }

    // Call only if the real DirectDraw SetDisplayMode succeeded.
    // This is a *diagnostic* epoch and does not change the game's objects.
    pub fn begin_mode(&mut self,mode:Mode) {
        self.epoch=self.epoch.saturating_add(1);
        self.mode=Some(mode);
        self.chain=Chain::default();
        self.preflight_reported=false;
    }

    // Returns (new evidence, complete chain now ready for first preflight).
    pub fn observe(&mut self,request:u32,parent:u32,attached:u32)->(bool,bool) {
        let changed=self.chain.observe(request,parent,attached);
        let ready=self.mode.is_some() && self.chain.three_d_ready()
            && !self.preflight_reported;
        if ready {self.preflight_reported=true;}
        (changed,ready)
    }
}


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

// Legacy Win32 (x86) DDSURFACEDESC2, 124 bytes / 31 DWORDs.
// The layout is defined by Wine's DirectDraw SDK header: DDPIXELFORMAT
// begins at DWORD 18 (byte offset 0x48), DDSCAPS2 at DWORD 26 (0x68).
// These are metadata only: the lpSurface pixel-data pointer is never read.
pub const DDSD_BACKBUFFERCOUNT:u32=0x20;
pub const DDSD_PIXELFORMAT:u32=0x1000;
pub const DDPF_RGB:u32=0x40;
pub const DDPF_ZBUFFER:u32=0x400;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SurfaceLayout {
    pub width:u32,
    pub height:u32,
    pub flags:u32,
    pub caps:u32,
    pub backbuffer_count:Option<u32>,
    pub pixel_flags:Option<u32>,
    pub bits_per_pixel:Option<u32>,
    pub color_masks:Option<[u32;4]>,
}

// No COM calls. This is safe on a *copied* fixed-size descriptor.
pub fn parse_surface_desc2(words:&[u32;31])->Option<SurfaceLayout> {
    if words[0]!=124{return None;}
    let flags=words[1];
    let pixel_flags=if flags&DDSD_PIXELFORMAT!=0 && words[18]==32 {
        Some(words[19])
    }else{None};
    let bits_per_pixel=pixel_flags.map(|_|words[21]);
    let color_masks=if pixel_flags.is_some_and(|pf|pf&DDPF_RGB!=0){
        Some([words[22],words[23],words[24],words[25]])
    }else{None};
    Some(SurfaceLayout{
        width:words[3],
        height:words[2],
        flags,
        caps:words[26],
        backbuffer_count:(flags&DDSD_BACKBUFFERCOUNT!=0).then_some(words[5]),
        pixel_flags,bits_per_pixel,color_masks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_legacy_124_byte_ddraw_description_and_16bit_rgb_masks(){
        let mut desc=[0u32;31];
        desc[0]=124;
        desc[1]=0x1027; // caps + width + height + backbuffer count + pixel format
        desc[2]=480;
        desc[3]=640;
        desc[5]=1;
        desc[18]=32;
        desc[19]=DDPF_RGB;
        desc[21]=16;
        desc[22]=0xf800;
        desc[23]=0x07e0;
        desc[24]=0x001f;
        desc[26]=0x1000e238;
        let got=parse_surface_desc2(&desc).unwrap();
        assert_eq!((got.width,got.height),(640,480));
        assert_eq!(got.backbuffer_count,Some(1));
        assert_eq!(got.bits_per_pixel,Some(16));
        assert_eq!(got.color_masks,Some([0xf800,0x07e0,0x1f,0]));
        assert_eq!(got.caps,0x1000e238);
    }
    #[test]
    fn preserve_zbuffer_depth_without_misreading_color_masks(){
        let mut d=[0u32;31];
        d[0]=124;
        d[1]=DDSD_PIXELFORMAT;
        d[18]=32;
        d[19]=DDPF_ZBUFFER;
        d[21]=16;
        d[26]=0x10024000;
        let info=parse_surface_desc2(&d).unwrap();
        assert_eq!(info.bits_per_pixel,Some(16));
        assert_eq!(info.color_masks,None);
        assert_eq!(info.backbuffer_count,None);
        d[0]=108;
        assert_eq!(parse_surface_desc2(&d),None);
    }
    #[test]
    fn absent_pixel_format_is_not_assumed_to_be_rgb(){
        let mut d=[0u32;31];
        d[0]=124;
        d[19]=DDPF_RGB;
        d[21]=16;
        let i=parse_surface_desc2(&d).unwrap();
        assert_eq!(i.pixel_flags,None);
        assert_eq!(i.bits_per_pixel,None);
        assert_eq!(i.color_masks,None);
    }
    #[test]
    fn new_display_mode_resets_prior_front_back_depth_evidence() {
        let mut session=Session::new();
        session.begin_mode(Mode{width:640,height:480,bpp:16});
        assert_eq!(session.epoch,1);
        assert_eq!(session.observe(BACKBUFFER,0x1000e238,0x1000601c),(true,false));
        assert_eq!(session.observe(ZBUFFER,0x1000601c,0x10024000),(true,true));
        assert_eq!(session.observe(ZBUFFER,0x1000601c,0x10024000),(false,false));
        session.begin_mode(Mode{width:800,height:600,bpp:16});
        assert_eq!(session.epoch,2);
        assert!(!session.chain.three_d_ready());
        assert_eq!(session.observe(BACKBUFFER,0x1000e238,0x1000601c),(true,false));
        assert_eq!(session.observe(ZBUFFER,0x1000601c,0x10024000),(true,true));
        session.begin_mode(Mode{width:800,height:600,bpp:16});
        assert_eq!(session.epoch,3);
        assert!(!session.chain.three_d_ready());
        assert!(!session.preflight_reported);
    }
    #[test]
    fn preflight_is_not_reported_without_a_successful_display_mode() {
        let mut session=Session::new();
        assert_eq!(session.observe(BACKBUFFER,0x1000e238,0x1000601c),(true,false));
        assert_eq!(session.observe(ZBUFFER,0x1000601c,0x10024000),(true,false));
        assert!(session.chain.three_d_ready());
        assert!(!session.preflight_reported);
        session.begin_mode(Mode{width:640,height:480,bpp:16});
        assert!(!session.chain.three_d_ready());
    }
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
