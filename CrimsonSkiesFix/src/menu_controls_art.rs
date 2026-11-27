//! Textured controller artwork, bounded and authenticated. The generated
//! asset bytes are appended to the unsigned experimental launcher by the
//! companion packer after CI. No game asset, font, or new dependency.
use std::{ffi::c_void,io::{Read,Seek,SeekFrom},ptr,sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}}};
use crate::borderless_native_blt::Rect;
type H=*mut c_void;
const PACK_SHA:&str="e9bbe0989507242bb64ca84722e9308d2e9308795c7e5782553e0174016f2dd4";
const PACK_LEN:usize=4177183;
const W:usize=1600;
const HGT:usize=780;
const RAW:usize=W*HGT*4;
const MAGIC:&[u8;8]=b"CSFART10";
const END:&[u8;8]=b"CSFEND10";
static PACK:OnceLock<Option<Vec<u8>>>=OnceLock::new();
static CACHE:OnceLock<Mutex<Option<Art>>>=OnceLock::new();
static HD_LOG:AtomicBool=AtomicBool::new(false);
static LOW_LOG:AtomicBool=AtomicBool::new(false);
struct Art{family:u8,lang:u8,pixels:Vec<u8>}
fn le32(s:&[u8])->Option<u32>{Some(u32::from_le_bytes(s.try_into().ok()?))}
fn ranges(p:&[u8])->Option<[(usize,usize);2]>{
    if p.get(..8)?!=MAGIC||le32(p.get(8..12)?)?!=W as u32||
        le32(p.get(12..16)?)?!=HGT as u32||le32(p.get(16..20)?)?!=2{return None;}
    let mut pos=20usize;let mut r=[(0,0);2];
    for out in &mut r{
        let start=pos.checked_add(4)?;
        let len=le32(p.get(pos..start)?)? as usize;
        if len==0||len>RAW{return None;}
        let end=start.checked_add(len)?;
        p.get(start..end)?;
        *out=(start,end);pos=end;
    }
    (pos==p.len()).then_some(r)
}
fn load()->Option<Vec<u8>>{
    let path=std::env::var_os("CRIMSONSKIESFIX_ART_LAUNCHER")?;
    let mut f=std::fs::File::open(path).ok()?;
    let meta=f.metadata().ok()?;
    if !meta.is_file()||meta.len()<(PACK_LEN+16) as u64||meta.len()>64*1024*1024{return None;}
    f.seek(SeekFrom::End(-16)).ok()?;
    let mut tail=[0u8;16];f.read_exact(&mut tail).ok()?;
    if &tail[..8]!=END||u64::from_le_bytes(tail[8..].try_into().ok()?)!=PACK_LEN as u64{return None;}
    f.seek(SeekFrom::End(-((PACK_LEN+16) as i64))).ok()?;
    let mut p=vec![0u8;PACK_LEN];f.read_exact(&mut p).ok()?;
    if crate::exe_gate::sha256(&p)!=PACK_SHA||ranges(&p).is_none(){return None;}
    Some(p)
}
fn pack()->Option<&'static [u8]>{
    PACK.get_or_init(||{
        let p=load();
        crate::log(if p.is_some(){"MENU-CONTROLS-ART: authenticated generated 1600x780 artwork loaded"}else{"MENU-CONTROLS-ART: no authenticated overlay; prior generic/vector fallback retained"});
        p
    }).as_deref()
}
pub fn available()->bool{pack().is_some()}
fn unpack(p:&[u8],family:u8)->Option<Vec<u8>>{
    let r=ranges(p)?;
    let (a,b)=*r.get(match family{2=>0,3=>1,_=>return None})?;
    let mut data=Vec::with_capacity(RAW);
    flate2::read::ZlibDecoder::new(p.get(a..b)?).take((RAW+1) as u64).read_to_end(&mut data).ok()?;
    (data.len()==RAW).then_some(data)
}
#[repr(C)]struct BitmapInfo{size:u32,width:i32,height:i32,planes:u16,bits:u16,compression:u32,image_size:u32,xppm:i32,yppm:i32,used:u32,important:u32}
fn info()->BitmapInfo{BitmapInfo{size:40,width:W as i32,height:-(HGT as i32),planes:1,bits:32,compression:0,image_size:RAW as u32,xppm:0,yppm:0,used:0,important:0}}
#[repr(C)]struct Size{cx:i32,cy:i32}
#[link(name="gdi32")]
extern "system"{
    fn CreateCompatibleDC(dc:H)->H;
    fn DeleteDC(dc:H)->i32;
    fn CreateDIBSection(dc:H,info:*const BitmapInfo,usage:u32,bits:*mut H,section:H,offset:u32)->H;
    fn SelectObject(dc:H,obj:H)->H;
    fn DeleteObject(obj:H)->i32;
    fn CreateFontW(height:i32,width:i32,escapement:i32,orientation:i32,weight:i32,italic:u32,underline:u32,strikeout:u32,charset:u32,out_precision:u32,clip_precision:u32,quality:u32,pitch_and_family:u32,face:*const u16)->H;
    fn SetBkMode(dc:H,mode:i32)->i32;
    fn SetTextColor(dc:H,c:u32)->u32;
    fn GetTextExtentPoint32W(dc:H,t:*const u16,len:i32,size:*mut Size)->i32;
    fn GdiFlush()->i32;
    fn SaveDC(dc:H)->i32;
    fn RestoreDC(dc:H,saved:i32)->i32;
    fn ExcludeClipRect(dc:H,l:i32,t:i32,r:i32,b:i32)->i32;
    fn SetStretchBltMode(dc:H,mode:i32)->i32;
    fn StretchDIBits(dc:H,x:i32,y:i32,w:i32,h:i32,sx:i32,sy:i32,sw:i32,sh:i32,bits:*const c_void,info:*const BitmapInfo,usage:u32,rop:u32)->i32;
}
#[link(name="user32")]
extern "system"{fn DrawTextW(dc:H,text:*const u16,count:i32,rect:*mut Rect,flags:u32)->i32;}
fn rect(l:i32,t:i32,r:i32,b:i32)->Rect{Rect{left:l,top:t,right:r,bottom:b}}
fn scaled(r:Rect)->Rect{rect(r.left*2,r.top*2,r.right*2,r.bottom*2)}
const GOLD:u32=0x009de1ff;
const LIGHT:u32=0x00c5e8ff;
const SHADOW:u32=0x00080a11;
fn tr<'a>(lang:u8,en:&'a str,fr:&'a str,de:&'a str)->&'a str{match lang{2=>fr,3=>de,_=>en}}
unsafe fn text_fit(dc:H,logical:Rect,t:&str,max:i32,center:bool,color:u32,bold:bool)->bool{
    let r=scaled(logical);
    let s:Vec<u16>=t.encode_utf16().collect();
    let face:Vec<u16>=if bold{"Arial Narrow"}else{"Arial"}.encode_utf16().chain([0]).collect();
    for px in (9..=max).rev(){
        let font=CreateFontW(-px*2,0,0,0,if bold{700}else{400},0,0,0,1,0,0,4,0,face.as_ptr());
        if font.is_null(){return false;}
        let old=SelectObject(dc,font);
        if old.is_null()||old as isize == -1{DeleteObject(font);return false;}
        let mut size=Size{cx:0,cy:0};
        let fits=GetTextExtentPoint32W(dc,s.as_ptr(),s.len() as i32,&mut size)!=0
            &&size.cx<=r.right-r.left-4&&size.cy<=r.bottom-r.top;
        let mut ok=false;
        if fits{
            let flags=0x20|0x4|0x800|if center{1}else{0};
            SetBkMode(dc,1);
            let mut shadow=rect(r.left+2,r.top+2,r.right+2,r.bottom+2);
            SetTextColor(dc,SHADOW);
            let a=DrawTextW(dc,s.as_ptr(),s.len() as i32,&mut shadow,flags)>0;
            let mut dst=r;SetTextColor(dc,color);
            ok=a&&DrawTextW(dc,s.as_ptr(),s.len() as i32,&mut dst,flags)>0;
        }
        SelectObject(dc,old);DeleteObject(font);
        if fits{return ok;}
    }
    false // never silently truncate a label
}
unsafe fn lettering(dc:H,family:u8,lang:u8)->bool{
    let ps=family==3;
    let t=|e,f,d|tr(lang,e,f,d);
    let mut ok=true;
    let mut draw=|r,s:&str,n,c,col,bold|{ok&=text_fit(dc,r,s,n,c,col,bold);};
    draw(rect(283,27,517,58),t("GAMEPAD CONTROLS","COMMANDES - MANETTE","GAMEPAD-STEUERUNG"),22,true,GOLD,true);
    let pause=if ps{"Options"}else{"Start"};
    draw(rect(337,70,463,93),&format!("{pause} : {}",t("Pause","Pause","Pause")),14,true,LIGHT,true);
    let left_ps=[("L2",t("Fire rockets","Tir roquettes","Raketenfeuer")),("L1",t("Previous enemy","Ennemi précédent","Voriger Gegner")),("D-PAD",t("Menu cursor","Curseur menu","Menüzeiger")),("LS",t("Pitch / Roll","Tangage / roulis","Nicken / Rollen")),("L3",t("Cycle rockets","Changer roquettes","Raketen wechseln"))];
    let left_xbox=[("LT",left_ps[0].1),("LB",left_ps[1].1),("LS",left_ps[3].1),("LS↓",left_ps[4].1),("D-PAD",left_ps[2].1)];
    for (i,(head,action)) in (if ps{left_ps}else{left_xbox}).into_iter().enumerate(){
        let y=76+i as i32*35;
        draw(rect(38,y+5,83,y+25),head,13,true,LIGHT,true);
        draw(rect(90,y+3,230,y+27),action,14,false,GOLD,true);
    }
    let right=[(if ps{"R2"}else{"RT"},t("Fire guns","Tir mitrailleuses","MG-Feuer")),
        (if ps{"R1"}else{"RB"},t("Next enemy","Ennemi suivant","Nächster Gegner")),
        ("Y",t("Target attacker","Cibler l'attaquant","Angreifer wählen")),
        ("B",t("Nitro","Nitro","Nitro")),
        ("A",t("Cycle guns","Changer d'arme","Waffen wechseln")),
        ("X",t("Cycle views","Changer de vue","Ansicht wechseln"))];
    for (i,(head,action)) in right.into_iter().enumerate(){
        let y=76+i as i32*35;
        if !ps||i<2{
            let color=if i<2{LIGHT}else{[0x0044c9f3,0x00515ddd,0x006ebf63,0x00e0a669][i-2]};
            draw(rect(572,y+5,617,y+25),head,14,true,color,true);
        }
        draw(rect(624,y+3,764,y+27),action,14,false,GOLD,true);
    }
    draw(rect(285,307,515,321),t("RIGHT STICK: Yaw / Throttle","STICK DROIT : Lacet / gaz","RECHTER STICK: Gieren / Gas"),14,true,GOLD,true);
    draw(rect(285,321,515,334),t("Up: increase / Down: decrease","Haut : augmenter / Bas : réduire","Oben: mehr / Unten: weniger"),12,true,LIGHT,false);
    let (south,east)=if ps{(t("Cross","Croix","Kreuz"),t("Circle","Rond","Kreis"))}else{("A","B")};
    draw(rect(35,336,765,350),&format!("{}: {} = {}   {} = {}",t("MENUS","MENUS","MENÜS"),south,t("Select","Valider","Auswählen"),east,t("Back","Retour","Zurück")),13,true,LIGHT,false);
    draw(rect(35,350,765,365),&format!("{}: {} / {} / {} = {}",t("CINEMATICS","CINÉMATIQUES","ZWISCHENSEQUENZEN"),south,east,pause,t("Skip","Passer","Überspringen")),13,true,LIGHT,false);
    draw(rect(215,365,585,385),t("PRESS ANY KEY OR BUTTON TO RETURN","UNE TOUCHE OU UN BOUTON POUR REVENIR","BELIEBIGE TASTE ZUM SCHLIESSEN"),13,true,GOLD,true);
    ok
}
unsafe fn render(dc:H,family:u8,lang:u8)->Option<Art>{
    let raw=unpack(pack()?,family)?;
    let mem=CreateCompatibleDC(dc);if mem.is_null(){return None;}
    let mut bits:H=ptr::null_mut();let dib=CreateDIBSection(mem,&info(),0,&mut bits,ptr::null_mut(),0);
    if dib.is_null()||bits.is_null(){if !dib.is_null(){DeleteObject(dib);}DeleteDC(mem);return None;}
    let old=SelectObject(mem,dib);
    if old.is_null()||old as isize == -1{DeleteObject(dib);DeleteDC(mem);return None;}
    ptr::copy_nonoverlapping(raw.as_ptr(),bits.cast::<u8>(),RAW);
    let ok=lettering(mem,family,lang)&&GdiFlush()!=0;
    let out=if ok{Some(Art{family,lang,pixels:std::slice::from_raw_parts(bits.cast::<u8>(),RAW).to_vec()})}else{None};
    SelectObject(mem,old);DeleteObject(dib);DeleteDC(mem);
    out
}
fn scale(origin:i32,extent:i32,logical:i32,denom:i32)->i32{origin+((i64::from(extent)*i64::from(logical))/i64::from(denom)) as i32}
fn layout(view:Rect)->Option<(Rect,Rect)>{
    let w=view.right.checked_sub(view.left)?;let h=view.bottom.checked_sub(view.top)?;
    if w<800||h<600||w>8192||h>4320{return None;}
    let x=|n|scale(view.left,w,n,800);let y=|n|scale(view.top,h,n,600);
    // Keep the genuine skull/wing emblem at top-center. The native
    // Controls tab at x110..337 is fully concealed by the textured area.
    Some((rect(x(0),y(210),x(800),y(600)),rect(x(338),y(210),x(466),y(240))))
}
pub unsafe fn draw(dc:H,view:Rect,family:u8,lang:u8)->bool{
    if dc.is_null()||!matches!(family,2|3){return false;}
    let Some((dst,hole))=layout(view)else{return false;};
    let Ok(mut slot)=CACHE.get_or_init(||Mutex::new(None)).lock()else{return false;};
    if slot.as_ref().is_none_or(|a|a.family!=family||a.lang!=lang){
        let Some(a)=render(dc,family,lang)else{return false;};*slot=Some(a);
    }
    let Some(a)=slot.as_ref()else{return false;};
    let saved=SaveDC(dc);if saved==0{return false;}
    let ok=ExcludeClipRect(dc,hole.left,hole.top,hole.right,hole.bottom)!=0&&
        SetStretchBltMode(dc,4)!=0&&
        StretchDIBits(dc,dst.left,dst.top,dst.right-dst.left,dst.bottom-dst.top,0,0,W as i32,HGT as i32,a.pixels.as_ptr().cast(),&info(),0,0x00cc0020)>0;
    let restored=RestoreDC(dc,saved)!=0;
    if ok&&restored&&view.right-view.left==800&&!LOW_LOG.swap(true,Ordering::AcqRel){
        crate::log("MENU-CONTROLS-ART: textured artwork composited on legacy scratch (HD final compositor optional)");
    }
    ok&&restored
}
pub fn note_hd(view:Rect){
    if !HD_LOG.swap(true,Ordering::AcqRel){
        crate::log(&format!("MENU-CONTROLS-HD: final textured artwork composed at {}x{} 4:3 viewport",view.right-view.left,view.bottom-view.top));
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn exact_bounded_container_structure(){
        let mut p=MAGIC.to_vec();p.extend((W as u32).to_le_bytes());p.extend((HGT as u32).to_le_bytes());p.extend(2u32.to_le_bytes());
        for v in [b"abc",b"def"]{p.extend(3u32.to_le_bytes());p.extend(v);}
        assert_eq!(ranges(&p),Some([(24,27),(31,34)]));
        assert!(unpack(&p,2).is_none());
        p.push(0);assert!(ranges(&p).is_none());p.pop();p[20]=255;assert!(ranges(&p).is_none());
    }
    #[test]fn full_panel_and_emblem_cutout_scale_with_native_viewport(){
        assert_eq!(layout(rect(0,0,800,600)),Some((rect(0,210,800,600),rect(338,210,466,240))));
        assert_eq!(layout(rect(240,0,1680,1080)),Some((rect(240,378,1680,1080),rect(848,378,1078,432))));
        assert!(layout(rect(0,0,640,480)).is_none());
    }
}
