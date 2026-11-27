//! Modal static controls diagram, with active SDL3 controller artwork.
//! Only the exact observed Controls Preferences page; original UI remains.
use std::{ffi::c_void,ptr,sync::{Mutex,OnceLock,atomic::{AtomicU8,Ordering}},time::{Duration,Instant}};
use crate::borderless_native_blt::Rect;
type H=*mut c_void;
const NONE:u8=0;
const CANDIDATE:u8=1;
const CONTROLS:u8=2;
static PAGE:AtomicU8=AtomicU8::new(NONE);
static FAMILY:AtomicU8=AtomicU8::new(0);
// Low two bits are the first detected mapped pad family; high bits
// are the mask of available families. Independent of pilot ownership.
static INVENTORY:AtomicU8=AtomicU8::new(0);
fn preferred_family(active:u8,inventory:u8)->u8{
    if active!=0{active}else{inventory&3}
}
fn display_family()->u8{preferred_family(FAMILY.load(Ordering::Acquire),INVENTORY.load(Ordering::Acquire))}
fn mixed_unselected()->bool{
    FAMILY.load(Ordering::Acquire)==0&&(INVENTORY.load(Ordering::Acquire)>>2).count_ones()>1
}
pub fn detected_family(kind:i32,name:&str)->u8{family_for(kind,name)}
pub fn update_inventory(first:u8,mask:u8){
    let state=(first&3)|((mask&15)<<2);
    let old=INVENTORY.swap(state,Ordering::AcqRel);
    if state!=old&&crate::cfg().log_enabled{
        crate::log(&format!("MENU-CONTROLS-PANEL: detected families mask={mask:#x} preferred={first}; independent of active device"));
    }
    if state==0&&old!=0{fail_preview();}
}

static LANGUAGE:AtomicU8=AtomicU8::new(0);
static RECEIPT_LANGUAGE:OnceLock<u8>=OnceLock::new();
const GENERIC:u8=1;
const XBOX:u8=2;
const PS:u8=3;
const OPEN_BUTTON:u32=1<<3;
struct Input{
    open:bool,buttons:u32,axis:bool,pad_block:bool,mouse_block:bool,
    keys:[bool;256],present:Option<Instant>,
}
impl Default for Input{
    fn default()->Self{Self{open:false,buttons:0,axis:false,pad_block:false,mouse_block:false,keys:[false;256],present:None}}
}
static INPUT:OnceLock<Mutex<Input>>=OnceLock::new();
fn input()->&'static Mutex<Input>{INPUT.get_or_init(||Mutex::new(Input::default()))}
fn close(s:&mut Input,reason:&str){
    if s.open{
        s.open=false;s.present=None;
        crate::menu_controls_art::invalidate();
        if crate::cfg().log_enabled{crate::log(&format!("MENU-CONTROLS-PANEL: closed reason={reason}"));}
    }
}
fn family_for(kind:i32,name:&str)->u8{
    match kind{
        2|3=>XBOX,4|5|6=>PS,
        0|1=>{
            let name=name.to_ascii_lowercase();
            if name.contains("dualshock")||name.contains("dualsense")||name.contains("ps4 controller")||name.contains("ps5 controller")||name.contains("playstation"){PS}
            else if name.contains("xbox"){XBOX}else{GENERIC}
        },
        _=>GENERIC,
    }
}
pub fn device_selected(kind:i32,name:&str){
    let family=family_for(kind,name);
    FAMILY.store(family,Ordering::Release);
    if let Ok(mut s)=input().lock(){takeover(&mut s);}
    if crate::cfg().log_enabled{crate::log(&format!("MENU-CONTROLS-PANEL: active family={family} SDL type={kind}"));}
}
fn takeover(s:&mut Input){
    // Selection happens before the new controller's first sample. Carry
    // the modal dismissal block across the change, until neutral.
    let block=s.open||s.pad_block;
    close(s,"controller-change");
    s.buttons=0;s.axis=false;s.pad_block=block;
}
pub fn device_removed(){
    FAMILY.store(0,Ordering::Release);
    if let Ok(mut s)=input().lock(){takeover(&mut s);}
}
pub fn reset_input(){
    if let Ok(mut s)=input().lock(){close(&mut s,"focus-reset");*s=Input::default();}
}
fn context()->bool{page_active()&&crate::menu_gos_event::preferences_transaction_active()&&crate::menu_video_probe::legacy_ui_active()}
fn open_panel(s:&mut Input,reason:&str){
    s.open=true;s.pad_block=true;s.present=Some(Instant::now());
    crate::menu_controls_art::invalidate();
    if crate::cfg().log_enabled{crate::log(&format!("MENU-CONTROLS-PANEL: opened by {reason}"));}
}
fn expire_preview(s:&mut Input,now:Instant){
    if s.open&&s.present.is_some_and(|t|now.saturating_duration_since(t)>Duration::from_secs(2)){
        close(s,"presentation-timeout");
    }
}
fn leave_controls(s:&mut Input){
    close(s,"page-exit");
    s.mouse_block=false;
    s.keys.fill(false);
    s.pad_block=false;
}
fn pad_step(s:&mut Input,buttons:u32,axis:bool,neutral:bool,eligible:bool)->bool{
    let pressed=buttons&!s.buttons;
    let moved=axis&&!s.axis;
    s.buttons=buttons;s.axis=axis;
    if !eligible{leave_controls(s);return false;}
    if s.open{
        if pressed!=0||moved{close(s,if pressed!=0{"gamepad-button"}else{"gamepad-axis"});s.pad_block=true;}
        return true;
    }
    if s.pad_block{
        if neutral{s.pad_block=false;}
        return true;
    }
    if pressed==OPEN_BUTTON&&buttons==OPEN_BUTTON&&!axis{
        open_panel(s,"active gamepad");
        return true;
    }
    false
}
pub fn pad_event(buttons:u32,axis:bool,neutral:bool,eligible:bool)->bool{
    let Ok(mut s)=input().lock()else{return false;};
    expire_preview(&mut s,Instant::now());
    pad_step(&mut s,buttons,axis,neutral,eligible&&context()&&FAMILY.load(Ordering::Acquire)!=0)
}
// Match the exact painted button, not any underlying native widget.
// GOS cursor coordinates are in the original 800x600 logical viewport.
const INVITE_LEFT:i32=300;
const INVITE_TOP:i32=516;
const INVITE_RIGHT:i32=606;
const INVITE_BOTTOM:i32=552;
fn invite_hit(x:i32,y:i32)->bool{
    (INVITE_LEFT..INVITE_RIGHT).contains(&x)&&(INVITE_TOP..INVITE_BOTTOM).contains(&y)
}
fn mouse_step(s:&mut Input,buttons:u32,moved:bool,open_click:bool)->bool{
    // Consume the opening click and its release before accepting another
    // dismissal, including movement while the opening button is held.
    if s.mouse_block{
        if buttons==0{s.mouse_block=false;}
        return true;
    }
    if s.open{
        if buttons!=0||moved{close(s,if buttons!=0{"mouse-button"}else{"mouse-motion"});s.mouse_block=buttons!=0;}
        return true;
    }
    if open_click&&buttons==0x80{
        open_panel(s,"native cursor click");s.mouse_block=true;return true;
    }
    false
}
pub fn mouse_event(buttons:u32,moved:bool,x:i32,y:i32,left_edge:bool)->bool{
    if !context(){return false;}
    let Ok(mut s)=input().lock()else{return false;};
    expire_preview(&mut s,Instant::now());
    let open_click=display_family()!=0&&LAST_PRESENT.load(Ordering::Acquire)==1&&left_edge&&invite_hit(x,y);
    mouse_step(&mut s,buttons,moved,open_click)
}
pub fn keyboard_capture()->bool{
    context()&&input().lock().is_ok_and(|mut s|{expire_preview(&mut s,Instant::now());s.open||s.keys.iter().any(|&v|v)})
}
pub fn keyboard_event(scan:usize,pressed:bool){
    if scan>=256{return;}
    if let Ok(mut s)=input().lock(){
        s.keys[scan]=pressed;
        if pressed{close(&mut s,"keyboard");}
    }
}
pub fn is_open()->bool{context()&&input().lock().is_ok_and(|mut s|{expire_preview(&mut s,Instant::now());s.open})}
pub fn fail_preview(){if let Ok(mut s)=input().lock(){close(&mut s,"page-or-render-reset");}}
fn edition_language(s:&str)->u8{
    let mut ids=s.lines().filter_map(|l|l.strip_prefix("Edition="));
    let code=match ids.next(){Some("C_SKIES_FR_001"|"C_SKIES_FR_002")=>2,Some("C_SKIES_DE_001")=>3,_=>1};
    if ids.next().is_some(){1}else{code}
}
fn language()->u8{
    let observed=LANGUAGE.load(Ordering::Acquire);
    if observed!=0{return observed;}
    *RECEIPT_LANGUAGE.get_or_init(||{
        let text=std::env::current_exe().ok().and_then(|p|p.parent()?.parent().map(|p|p.join("state").join("CrimsonSkiesFix.install-receipt.txt")))
            .filter(|p|std::fs::metadata(p).is_ok_and(|m|m.is_file()&&m.len()<65536))
            .and_then(|p|std::fs::read_to_string(p).ok());
        text.as_deref().map_or(1,edition_language)
    })
}
pub fn observe_title(bytes:&[u8]){
    let end=bytes.iter().position(|&b|b==0).unwrap_or(bytes.len());
    let t=String::from_utf8_lossy(&bytes[..end]).trim().to_ascii_lowercase();
    let code=if t=="steuerung"{3}else if t=="controls"||t=="control"{1}
        else if t=="commandes"||t=="contrôles"||t=="contr�les"{2}else{0};
    if code!=0{LANGUAGE.store(code,Ordering::Release);}
}
fn tr<'a>(en:&'a str,fr:&'a str,de:&'a str)->&'a str{
    match language(){2=>fr,3=>de,_=>en}
}
struct Labels{lt:&'static str,lb:&'static str,rt:&'static str,rb:&'static str,north:&'static str,east:&'static str,south:&'static str,west:&'static str,start:&'static str,face:[&'static str;4]}
fn labels(family:u8)->Labels{
    match family{
        PS=>Labels{lt:"L2",lb:"L1",rt:"R2",rb:"R1",north:tr("Triangle","Triangle","Dreieck"),east:tr("Circle","Rond","Kreis"),south:tr("Cross","Croix","Kreuz"),west:tr("Square","Carré","Quadrat"),start:"Options",face:["△","○","□","×"]},
        XBOX=>Labels{lt:"LT",lb:"LB",rt:"RT",rb:"RB",north:"Y",east:"B",south:"A",west:"X",start:"Start",face:["Y","B","X","A"]},
        _=>Labels{lt:"LT",lb:"LB",rt:"RT",rb:"RB",north:tr("Top","Haut","Oben"),east:tr("Right","Droite","Rechts"),south:tr("Bottom","Bas","Unten"),west:tr("Left","Gauche","Links"),start:"Start",face:["N","E","W","S"]},
    }
}
static DRAW_KIND:AtomicU8=AtomicU8::new(0);
static PENDING:AtomicU8=AtomicU8::new(0);
static LAST_PRESENT:AtomicU8=AtomicU8::new(0);
static SEEN:AtomicU8=AtomicU8::new(0);
fn next_page(old:u8,name:&str)->u8{
    match name{
        "cp_background"=>CANDIDATE,
        "cp_t_joysticktitle" if matches!(old,CANDIDATE|CONTROLS)=>CONTROLS,
        "mm_logo"=>NONE,
        _ if name.ends_with("_background")=>NONE,
        _=>old,
    }
}
pub fn observe_member(name:&str){
    let before=PAGE.load(Ordering::Acquire);
    let after=next_page(before,name);
    if before!=after{
        PAGE.store(after,Ordering::Release);
        if after!=CONTROLS{
            if let Ok(mut s)=input().lock(){leave_controls(&mut s);}
            not_presented();
        }
        if after==CONTROLS&&crate::cfg().log_enabled{
            crate::log("MENU-CONTROLS-PANEL: verified cp_background + cp_t_joysticktitle; preview available");
        }
    }
}
pub fn clear_page(){
    if PAGE.swap(NONE,Ordering::AcqRel)!=NONE{
        if let Ok(mut s)=input().lock(){leave_controls(&mut s);}
    }
    not_presented();
}
pub fn page_active()->bool{PAGE.load(Ordering::Acquire)==CONTROLS}
#[link(name="user32")]
extern "system"{
    fn GetForegroundWindow()->H;
    fn GetWindowThreadProcessId(window:H,pid:*mut u32)->u32;
    fn FillRect(dc:H,rect:*const Rect,brush:H)->i32;
    fn DrawTextW(dc:H,text:*const u16,count:i32,rect:*mut Rect,flags:u32)->i32;
}
#[link(name="kernel32")]
extern "system"{fn GetCurrentProcessId()->u32;}
#[link(name="gdi32")]
extern "system"{
    fn SaveDC(dc:H)->i32;
    fn RestoreDC(dc:H,saved:i32)->i32;
    fn CreateSolidBrush(color:u32)->H;
    fn CreatePen(style:i32,width:i32,color:u32)->H;
    fn SelectObject(dc:H,obj:H)->H;
    fn DeleteObject(obj:H)->i32;
    fn SetBkMode(dc:H,mode:i32)->i32;
    fn SetTextColor(dc:H,color:u32)->u32;
    fn CreateFontW(height:i32,width:i32,escapement:i32,orientation:i32,weight:i32,
        italic:u32,underline:u32,strikeout:u32,charset:u32,out_precision:u32,
        clip_precision:u32,quality:u32,pitch_and_family:u32,face:*const u16)->H;
    fn Ellipse(dc:H,l:i32,t:i32,r:i32,b:i32)->i32;
    fn RoundRect(dc:H,l:i32,t:i32,r:i32,b:i32,ew:i32,eh:i32)->i32;
    fn Polygon(dc:H,points:*const Point,count:i32)->i32;
    fn MoveToEx(dc:H,x:i32,y:i32,old:*mut Point)->i32;
    fn BeginPath(dc:H)->i32;
    fn EndPath(dc:H)->i32;
    fn CloseFigure(dc:H)->i32;
    fn PolyBezierTo(dc:H,points:*const Point,count:u32)->i32;
    fn StrokeAndFillPath(dc:H)->i32;
    fn AbortPath(dc:H)->i32;
    fn CreateCompatibleDC(dc:H)->H;
    fn DeleteDC(dc:H)->i32;
    fn CreateDIBSection(dc:H,info:*const BitmapInfo,usage:u32,bits:*mut H,section:H,offset:u32)->H;
    fn GdiFlush()->i32;
    fn StretchDIBits(dc:H,x:i32,y:i32,w:i32,h:i32,sx:i32,sy:i32,sw:i32,sh:i32,bits:*const c_void,info:*const BitmapInfo,usage:u32,rop:u32)->i32;
    fn LineTo(dc:H,x:i32,y:i32)->i32;
}
#[repr(C)]#[derive(Clone,Copy)]struct Point{x:i32,y:i32}
#[repr(C)]struct BitmapInfo{size:u32,width:i32,height:i32,planes:u16,bits:u16,compression:u32,image_size:u32,xppm:i32,yppm:i32,used:u32,important:u32}
fn bitmap_info()->BitmapInfo{BitmapInfo{size:40,width:800,height:-360,planes:1,bits:32,compression:0,image_size:800*360*4,xppm:0,yppm:0,used:0,important:0}}
struct Artwork{family:u8,lang:u8,pixels:Vec<u32>}
static ART:OnceLock<Mutex<Option<Artwork>>>=OnceLock::new();
fn rect(l:i32,t:i32,r:i32,b:i32)->Rect{Rect{left:l,top:t,right:r,bottom:b}}
const fn rgb(r:u32,g:u32,b:u32)->u32{r|(g<<8)|(b<<16)}
const INK:u32=rgb(17,12,10);
const PAPER:u32=rgb(84,13,16);
const PANEL:u32=rgb(47,42,37);
const EDGE:u32=rgb(155,113,55);
const DARK:u32=rgb(23,21,20);
const LIGHT:u32=rgb(246,219,158);
const GOLD:u32=rgb(223,176,77);
const STEEL:u32=rgb(103,99,89);
const SHINE:u32=rgb(172,159,131);
const RED:u32=rgb(106,17,20);
const BRASS_DARK:u32=rgb(91,58,27);
fn mix(a:u32,b:u32,t:u32,n:u32)->u32{
    let c=|shift:u32|(((a>>shift)&255)*(n-t)+((b>>shift)&255)*t)/n;
    rgb(c(0),c(8),c(16))
}
pub fn eligible(view:Rect)->bool{
    context()&&display_family()!=0&&view==rect(0,0,800,600)
}
unsafe fn foreground()->bool{
    let w=GetForegroundWindow();let mut pid=0;
    !w.is_null()&&GetWindowThreadProcessId(w,&mut pid)!=0&&pid==GetCurrentProcessId()
}
struct Painter{dc:H,ok:bool}
impl Painter{
    unsafe fn fill(&mut self,r:Rect,c:u32){
        let brush=CreateSolidBrush(c);
        if brush.is_null(){self.ok=false;return;}
        self.ok&=FillRect(self.dc,&r,brush)!=0;
        DeleteObject(brush);
    }
    unsafe fn with_shape(&mut self,c:u32,edge:u32,draw:impl FnOnce(H)->i32){
        let brush=CreateSolidBrush(c);let pen=CreatePen(0,2,edge);
        if brush.is_null()||pen.is_null(){
            self.ok=false;
        }else{
            let ob=SelectObject(self.dc,brush);let op=SelectObject(self.dc,pen);
            if ob.is_null()||op.is_null()||ob as isize == -1||op as isize == -1{self.ok=false;}
            else{self.ok&=draw(self.dc)!=0;}
            if !op.is_null()&&op as isize != -1{SelectObject(self.dc,op);}
            if !ob.is_null()&&ob as isize != -1{SelectObject(self.dc,ob);}
        }
        if !pen.is_null(){DeleteObject(pen);}
        if !brush.is_null(){DeleteObject(brush);}
    }
    unsafe fn rounded(&mut self,r:Rect,c:u32,radius:i32){
        self.with_shape(c,INK,|d|RoundRect(d,r.left,r.top,r.right,r.bottom,radius,radius));
    }
    unsafe fn circle(&mut self,x:i32,y:i32,r:i32,c:u32){
        self.with_shape(c,INK,|d|Ellipse(d,x-r,y-r,x+r,y+r));
    }
    unsafe fn poly(&mut self,pts:&[(i32,i32)],c:u32){
        let points:Vec<Point>=pts.iter().map(|&(x,y)|Point{x,y}).collect();
        self.with_shape(c,INK,|d|Polygon(d,points.as_ptr(),points.len() as i32));
    }
    unsafe fn line_color(&mut self,pts:&[(i32,i32)],color:u32,width:i32){
        if pts.len()<2{return;}
        let pen=CreatePen(0,width,color);if pen.is_null(){self.ok=false;return;}
        let old=SelectObject(self.dc,pen);
        if old.is_null()||old as isize == -1{self.ok=false;}else{
            self.ok&=MoveToEx(self.dc,pts[0].0,pts[0].1,ptr::null_mut())!=0;
            for &(x,y) in &pts[1..]{self.ok&=LineTo(self.dc,x,y)!=0;}
            SelectObject(self.dc,old);
        }
        DeleteObject(pen);
    }
    unsafe fn text(&mut self,r:Rect,s:&str,size:i32,bold:bool,align:u32,color:u32){
        let face:Vec<u16>=if bold{"Arial Narrow"}else{"Arial"}.encode_utf16().chain([0]).collect();
        let font=CreateFontW(-size,0,0,0,if bold{700}else{400},0,0,0,1,0,0,4,0,face.as_ptr());
        if font.is_null(){self.ok=false;return;}
        let old=SelectObject(self.dc,font);
        if old.is_null()||old as isize == -1{self.ok=false;}else{
            SetBkMode(self.dc,1);SetTextColor(self.dc,color);
            let text:Vec<u16>=s.encode_utf16().collect();let mut r=r;
            self.ok&=DrawTextW(self.dc,text.as_ptr(),text.len() as i32,&mut r,0x20|0x4|0x800|align)>0;
            SelectObject(self.dc,old);
        }
        DeleteObject(font);
    }
    unsafe fn plate(&mut self,r:Rect){
        self.rounded(r,INK,12);
        self.rounded(rect(r.left+1,r.top+1,r.right-1,r.bottom-1),BRASS_DARK,10);
        self.rounded(rect(r.left+2,r.top+2,r.right-2,r.bottom-2),GOLD,9);
        self.rounded(rect(r.left+4,r.top+4,r.right-4,r.bottom-4),PANEL,7);
        self.line_color(&[(r.left+9,r.top+4),(r.right-9,r.top+4)],SHINE,1);
        self.line_color(&[(r.left+9,r.bottom-5),(r.right-9,r.bottom-5)],DARK,1);
    }
    unsafe fn rivet(&mut self,x:i32,y:i32){
        self.circle(x,y,4,INK);self.circle(x,y,3,EDGE);
        self.line_color(&[(x-1,y-1),(x+1,y+1)],LIGHT,1);
    }
    unsafe fn label(&mut self,x:i32,y:i32,head:&str,body:&str){
        self.plate(rect(x,y,x+196,y+31));
        self.rounded(rect(x+7,y+6,x+54,y+25),DARK,6);
        self.text(rect(x+8,y+5,x+53,y+26),head,12,true,1,LIGHT);
        self.text(rect(x+58,y+4,x+190,y+27),body,13,true,0,LIGHT);
    }
    unsafe fn wire(&mut self,pts:&[(i32,i32)]){
        self.line_color(pts,INK,4);
        self.line_color(pts,GOLD,2);
        if let Some(&(x,y))=pts.last(){
            self.circle(x,y,4,BRASS_DARK);self.circle(x,y,3,GOLD);
            self.line_color(&[(x-1,y-1),(x+1,y-1)],LIGHT,1);
        }
    }
    unsafe fn shell(&mut self,ps:bool,c:u32,edge:u32,offset:i32){
        let start=Point{x:286,y:64+offset};
        let pts=if ps{[
            (313,50),(332,61),(352,65),(385,66),(415,66),(448,65),
            (468,61),(487,50),(514,64),(544,81),(556,135),(560,178),
            (566,211),(553,239),(532,240),(506,242),(487,211),(473,192),
            (455,179),(345,179),(327,192),(313,211),(294,242),(268,240),
            (247,239),(234,211),(240,178),(244,135),(256,81),(286,64),
        ]}else{[
            (313,50),(332,58),(352,62),(385,63),(415,63),(448,62),
            (468,58),(487,50),(514,64),(545,88),(558,131),(561,177),
            (567,211),(551,239),(530,240),(506,242),(485,210),(470,186),
            (451,174),(349,174),(330,186),(315,210),(294,242),(270,240),
            (249,239),(233,211),(239,177),(242,131),(255,88),(286,64),
        ]};
        let points:Vec<Point>=pts.into_iter().map(|(x,y)|Point{x,y:y+offset}).collect();
        self.with_shape(c,edge,|dc|{
            if BeginPath(dc)==0{return 0;}
            if MoveToEx(dc,start.x,start.y,ptr::null_mut())==0||
                PolyBezierTo(dc,points.as_ptr(),points.len() as u32)==0||
                CloseFigure(dc)==0||EndPath(dc)==0{
                AbortPath(dc);return 0;
            }
            StrokeAndFillPath(dc)
        });
    }
    unsafe fn stick(&mut self,x:i32,y:i32){
        self.circle(x,y,29,INK);self.circle(x,y,26,STEEL);
        self.circle(x,y,23,DARK);self.circle(x,y,19,INK);
        for r in (3..=17).rev(){
            let c=mix(rgb(31,29,27),rgb(76,72,65),(17-r) as u32,14);
            self.circle(x,y-1,r,c);
        }
        self.line_color(&[(x-11,y-13),(x-4,y-15),(x+5,y-15),(x+12,y-11)],SHINE,1);
    }
    unsafe fn face_symbol(&mut self,x:i32,y:i32,i:usize,ps:bool,t:&str){
        self.circle(x,y,13,INK);self.circle(x,y,11,STEEL);self.circle(x,y,9,DARK);
        let c=if ps{[rgb(92,214,177),rgb(228,91,92),rgb(215,118,190),rgb(115,170,235)][i]}
            else{[rgb(234,197,72),rgb(222,88,81),rgb(105,166,224),rgb(99,191,110)][i]};
        if ps{
            match i{
                0=>self.line_color(&[(x,y-6),(x-6,y+5),(x+6,y+5),(x,y-6)],c,2),
                1=>{self.with_shape(DARK,c,|d|Ellipse(d,x-6,y-6,x+6,y+6));},
                2=>self.line_color(&[(x-5,y-5),(x+5,y-5),(x+5,y+5),(x-5,y+5),(x-5,y-5)],c,2),
                _=>{self.line_color(&[(x-5,y-5),(x+5,y+5)],c,2);self.line_color(&[(x+5,y-5),(x-5,y+5)],c,2);}
            }
        }else{self.text(rect(x-9,y-9,x+9,y+9),t,13,true,1,c);}
    }
    // Cached 800x360 bottom panel, preserving the game's own title/emblem.
    unsafe fn panel(&mut self,family:u8){
        let b=labels(family);let ps=family==PS;
        self.fill(rect(0,0,800,360),PAPER);
        // Original procedural red leather/paint texture. Deterministic,
        // restrained noise; no external game graphics are distributed.
        for y in 0..360{
            let tone=if y<180{y}else{360-y};
            self.fill(rect(0,y,800,y+1),mix(rgb(43,7,10),RED,tone as u32,180));
        }
        self.rounded(rect(7,12,793,354),INK,24);
        self.rounded(rect(9,13,791,352),STEEL,22);
        self.rounded(rect(13,17,787,348),SHINE,20);
        self.rounded(rect(16,20,784,345),BRASS_DARK,18);
        self.rounded(rect(19,23,781,342),DARK,16);
        self.rounded(rect(23,27,777,338),PAPER,14);
        for y in 30..335{
            let tone=if y<180{y}else{360-y};
            self.fill(rect(27,y,773,y+1),mix(rgb(54,8,11),RED,tone as u32,180));
        }
        for i in 0..24{
            let x=35+i*32;
            for y in [20,345]{self.circle(x,y,5,INK);self.circle(x,y,3,PAPER);}
        }
        for (x,y) in [(16,25),(784,25),(16,339),(784,339)]{self.rivet(x,y);}
        self.plate(rect(224,1,576,38));
        self.text(rect(232,3,568,35),tr("GAMEPAD CONTROLS","COMMANDES - MANETTE","GAMEPAD-STEUERUNG"),23,true,1,LIGHT);
        self.text(rect(242,39,558,56),match family{PS=>"PLAYSTATION",XBOX=>"XBOX",_=>tr("STANDARD GAMEPAD","MANETTE STANDARD","STANDARD-GAMEPAD")},12,true,1,GOLD);
        // Rounded shoulder controls and layered, curved controller shell.
        for x in [282,468]{
            self.rounded(rect(x,47,x+52,82),INK,12);
            self.rounded(rect(x+2,49,x+50,72),STEEL,10);
            self.rounded(rect(x-5,68,x+57,91),DARK,9);
            self.line_color(&[(x+3,70),(x+49,70)],SHINE,1);
        }
        self.shell(ps,INK,INK,4);
        self.shell(ps,STEEL,INK,0);
        self.shell(ps,PANEL,SHINE,1);
        self.line_color(&[(270,101),(260,139),(253,179),(254,199)],STEEL,2);
        self.line_color(&[(530,101),(540,139),(547,179),(546,199)],STEEL,2);
        let (lx,ly,dx,dy,rx,ry,options)=if ps{
            self.rounded(rect(353,71,447,126),INK,10);
            self.rounded(rect(356,74,444,123),DARK,8);
            for y in (80..120).step_by(5){self.line_color(&[(362,y),(438,y)],PANEL,1);}
            self.rounded(rect(337,76,349,96),STEEL,5);
            self.rounded(rect(451,76,463,96),STEEL,5);
            self.circle(400,165,11,INK);self.circle(400,165,8,STEEL);
            (345,169,306,111,455,169,(457,86))
        }else{
            self.circle(400,83,13,INK);self.circle(400,83,10,STEEL);
            self.rounded(rect(374,112,390,125),DARK,5);
            self.rounded(rect(410,112,426,125),DARK,5);
            (310,111,345,169,455,169,(418,118))
        };
        self.circle(dx,dy,25,INK);self.circle(dx,dy,23,DARK);
        self.poly(&[(dx-7,dy-19),(dx+7,dy-19),(dx+7,dy-7),(dx+19,dy-7),
            (dx+19,dy+7),(dx+7,dy+7),(dx+7,dy+19),(dx-7,dy+19),
            (dx-7,dy+7),(dx-19,dy+7),(dx-19,dy-7),(dx-7,dy-7)],STEEL);
        self.stick(lx,ly);self.stick(rx,ry);
        for (i,((x,y),t)) in [(494,87),(514,111),(474,111),(494,135)].into_iter().zip(b.face).enumerate(){
            self.face_symbol(x,y,i,ps,t);
        }
        // Callout rows: left and right aligned, leaders terminate on the
        // actual controls. Route the west face key around the lower edge
        // rather than across the other face-button symbols.
        let l=28;let r=576;
        self.wire(&[(224,66),(255,66),(294,53)]);
        self.wire(&[(224,103),(246,103),(284,76)]);
        self.wire(&[(224,140),(245,140),(dx-19,dy)]);
        self.wire(&[(224,177),(249,177),(lx-19,ly+8)]);
        self.wire(&[(224,214),(265,214),(lx,ly+24)]);
        self.wire(&[(576,66),(545,66),(506,53)]);
        self.wire(&[(576,103),(552,103),(518,76)]);
        self.wire(&[(576,140),(549,140),(529,87),(506,87)]);
        self.wire(&[(576,177),(559,177),(527,111)]);
        self.wire(&[(576,214),(546,214),(507,148),(494,147)]);
        self.wire(&[(576,251),(524,251),(478,224),(474,123)]);
        self.wire(&[(224,251),(268,251),(rx,251),(rx,ry+29)]);
        self.wire(&[(400,261),(400,221),(427,208),(427,137),options]);
        self.label(l,50,b.lt,tr("Fire rockets","Tir roquettes","Raketenfeuer"));
        self.label(l,87,b.lb,tr("Previous enemy","Ennemi précédent","Voriger Gegner"));
        self.label(l,124,tr("D-PAD","CROIX","KREUZ"),tr("Menu cursor","Curseur menu","Menüzeiger"));
        self.label(l,161,tr("L STICK","STICK G","L STICK"),tr("Pitch / Roll","Tangage / roulis","Nicken / Rollen"));
        self.label(l,198,if ps{"L3"}else{"LS"},tr("Cycle rockets","Changer roquettes","Raketen wechseln"));
        self.label(l,235,tr("R STICK","STICK D","R STICK"),tr("Yaw / Throttle","Lacet / gaz","Gieren / Gas"));
        self.label(r,50,b.rt,tr("Fire guns","Tir mitrailleuses","MG-Feuer"));
        self.label(r,87,b.rb,tr("Next enemy","Ennemi suivant","Nächster Gegner"));
        self.label(r,124,b.north,tr("Target attacker","Cibler l'attaquant","Angreifer wählen"));
        self.label(r,161,b.east,tr("Nitro","Nitro","Nitro"));
        self.label(r,198,b.south,tr("Cycle guns","Changer d'arme","Waffen wechseln"));
        self.label(r,235,b.west,tr("Cycle views","Changer de vue","Ansicht wechseln"));
        self.plate(rect(306,260,494,283));
        let pause=format!("{} : {}",b.start,tr("Pause","Pause","Pause"));
        self.text(rect(311,261,489,282),&pause,13,true,1,LIGHT);
        let menus=format!("{}: {} = {}   {} = {}",tr("MENUS","MENUS","MENÜS"),b.south,tr("Select","Valider","Auswählen"),b.east,tr("Back","Retour","Zurück"));
        let movies=format!("{}: {} / {} / {} = {}",tr("CINEMATICS","CINÉMATIQUES","ZWISCHENSEQUENZEN"),b.south,b.east,b.start,tr("Skip","Passer","Überspringen"));
        self.text(rect(38,284,762,300),&menus,13,false,1,LIGHT);
        self.text(rect(38,301,762,317),&movies,13,false,1,LIGHT);
        self.plate(rect(214,319,586,341));
        self.text(rect(221,319,579,341),tr("PRESS ANY KEY OR BUTTON TO RETURN","UNE TOUCHE OU UN BOUTON POUR REVENIR","BELIEBIGE TASTE ZUM SCHLIESSEN"),13,true,1,LIGHT);
    }
}
unsafe fn cached_panel(dc:H,family:u8)->bool{
    let lang=language();
    let Ok(mut slot)=ART.get_or_init(||Mutex::new(None)).lock()else{return false;};
    if slot.as_ref().is_none_or(|a|a.family!=family||a.lang!=lang){
        let mem=CreateCompatibleDC(dc);if mem.is_null(){return false;}
        let info=bitmap_info();let mut bits:H=ptr::null_mut();
        let dib=CreateDIBSection(mem,&info,0,&mut bits,ptr::null_mut(),0);
        if dib.is_null()||bits.is_null(){if !dib.is_null(){DeleteObject(dib);}DeleteDC(mem);return false;}
        let old=SelectObject(mem,dib);
        if old.is_null()||old as isize == -1{DeleteObject(dib);DeleteDC(mem);return false;}
        let mut p=Painter{dc:mem,ok:true};p.panel(family);
        let flushed=GdiFlush()!=0;
        if p.ok&&flushed{
            let pixels=std::slice::from_raw_parts(bits.cast::<u32>(),800*360).to_vec();
            *slot=Some(Artwork{family,lang,pixels});
        }
        SelectObject(mem,old);DeleteObject(dib);DeleteDC(mem);
        if !p.ok||!flushed{return false;}
    }
    let Some(a)=slot.as_ref()else{return false;};
    StretchDIBits(dc,0,240,800,360,0,0,800,360,a.pixels.as_ptr().cast(),&bitmap_info(),0,0x00cc0020)>0

}
pub unsafe fn draw(dc:H,view:Rect)->bool{
    if dc.is_null()||!eligible(view){return false;}
    let saved=SaveDC(dc);if saved==0{return false;}
    if !foreground(){fail_preview();}
    let shown=is_open();
    let family=display_family();
    let mut p=Painter{dc,ok:true};
    if shown{
        p.ok=if matches!(family,PS|XBOX)&&crate::menu_controls_art::available(){
            crate::menu_controls_art::draw(dc,view,family,language()) || cached_panel(dc,family)
        }else{cached_panel(dc,family)};
    }
    else{
        // Single gold button attached to the existing native frame.
        // Its logical bounds are shared with the cursor hit test.
        let plaque=rect(INVITE_LEFT,INVITE_TOP,INVITE_RIGHT,INVITE_BOTTOM);
        let textured=crate::menu_controls_art::draw_native_decoration(dc);
        if !textured{p.plate(plaque);}
        let b=labels(family);
        if mixed_unselected(){
            p.text(rect(321,523,341,543),"Y",13,true,1,GOLD);
            p.line_color(&[(352,527),(346,539),(358,539),(352,527)],rgb(78,215,173),2);
        }else if family==PS{
            p.line_color(&[(342,526),(335,539),(349,539),(342,526)],rgb(78,215,173),2);
        }else{
            p.text(rect(321,523,364,543),b.north,14,true,1,if family==XBOX{GOLD}else{LIGHT});
        }
        let label=tr("GAMEPAD CONTROLS","COMMANDES MANETTE","GAMEPAD-STEUERUNG");
        if textured{
            p.text(rect(370,524,596,546),label,15,true,1,DARK);
            p.text(rect(370,523,596,545),label,15,true,1,rgb(255,235,174));
        }else{p.text(rect(370,523,596,545),label,15,true,1,LIGHT);}
    }
    let restored=RestoreDC(dc,saved)!=0;
    if p.ok&&restored{DRAW_KIND.store(if shown{2}else{1},Ordering::Release);true}else{fail_preview();false}
}
pub fn hd_candidate(view:Rect)->bool{
    view.right-view.left>800&&view.bottom-view.top>600&&is_open()&&matches!(display_family(),PS|XBOX)&&crate::menu_controls_art::available()
}
pub unsafe fn draw_hd_surface(surface:H,view:Rect)->bool{
    if surface.is_null()||!hd_candidate(view){return false;}
    let table=*(surface as *const *const usize);if table.is_null(){return false;}
    let get:unsafe extern "system" fn(H,*mut H)->i32=std::mem::transmute(*table.add(17));
    let release:unsafe extern "system" fn(H,H)->i32=std::mem::transmute(*table.add(26));
    let mut dc:H=ptr::null_mut();
    if get(surface,&mut dc)<0||dc.is_null(){return false;}
    let drawn=crate::menu_controls_art::draw(dc,view,display_family(),language());
    let released=release(surface,dc)>=0;
    if drawn&&released{crate::menu_controls_art::note_hd(view);true}else{false}
}
pub fn prepared(){PENDING.store(DRAW_KIND.load(Ordering::Acquire),Ordering::Release);}
pub fn not_presented(){PENDING.store(0,Ordering::Release);DRAW_KIND.store(0,Ordering::Release);LAST_PRESENT.store(0,Ordering::Release);}
pub fn note_presented(){
    let kind=PENDING.swap(0,Ordering::AcqRel);
    if kind==0{return;}
    if kind==2{if let Ok(mut s)=input().lock(){if s.open{s.present=Some(Instant::now());}}}
    let old=LAST_PRESENT.swap(kind,Ordering::AcqRel);
    if kind==2&&old!=2&&crate::cfg().log_enabled{
        // Bound diagnostic volume; first successful full panel presentation.
        let family=display_family();let bit=1u8<<family;
        if SEEN.fetch_or(bit,Ordering::AcqRel)&bit==0{crate::log(&format!("MENU-CONTROLS-PANEL: static 4:3 diagram presented family={family} language={} on private scratch",language()));}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn page_requires_both_observed_members(){
        assert_eq!(next_page(NONE,"cp_t_joysticktitle"),NONE);
        assert_eq!(next_page(NONE,"cp_background"),CANDIDATE);
        assert_eq!(next_page(CANDIDATE,"cp_t_joysticktitle"),CONTROLS);
        assert_eq!(next_page(CONTROLS,"cp_s_mouse"),CONTROLS);
        for name in ["kb_background","pf_background","vp_background","fc_background","mm_logo"]{
            assert_eq!(next_page(CONTROLS,name),NONE);
        }
    }
    #[test]fn family_and_receipt_language(){
        assert_eq!(family_for(5,"PS4 Controller"),PS);
        assert_eq!(family_for(2,"Controller"),XBOX);
        assert_eq!(family_for(1,"DualShock 4"),PS);
        assert_eq!(family_for(7,"Switch"),GENERIC);
        assert_eq!(edition_language("Edition=C_SKIES_DE_001\n"),3);
        assert_eq!(edition_language("Edition=C_SKIES_FR_002\n"),2);
        assert_eq!(edition_language("Edition=C_SKIES_DISC_001\n"),1);
        assert_eq!(edition_language("Edition=C_SKIES_FR_002\nEdition=C_SKIES_DE_001\n"),1);
    }
    #[test]fn modal_ignores_opening_release_and_consumes_next_press(){
        let mut s=Input::default();
        assert!(pad_step(&mut s,8,false,false,true));assert!(s.open);
        assert!(pad_step(&mut s,8,false,false,true));assert!(s.open);
        assert!(pad_step(&mut s,0,false,true,true));assert!(s.open);
        assert!(pad_step(&mut s,1,false,false,true));assert!(!s.open);
        assert!(pad_step(&mut s,1,false,false,true));assert!(!s.open);
        assert!(pad_step(&mut s,0,false,true,true));assert!(!s.open);
        assert!(!pad_step(&mut s,1,false,false,true));
    }
    #[test]fn modal_closes_on_analog_edge_or_page_exit(){
        let mut s=Input::default();
        assert!(pad_step(&mut s,8,false,false,true));
        assert!(pad_step(&mut s,0,false,true,true));assert!(s.open);
        assert!(pad_step(&mut s,0,true,false,true));assert!(!s.open);
        assert!(pad_step(&mut s,0,false,true,true));
        assert!(pad_step(&mut s,8,false,false,true));assert!(s.open);
        assert!(!pad_step(&mut s,8,false,false,false));assert!(!s.open);
    }
    #[test]fn takeover_consumes_first_gesture_until_neutral(){
        for button in [1,2,8,64]{
            let mut s=Input::default();
            assert!(pad_step(&mut s,8,false,false,true));
            assert!(pad_step(&mut s,0,false,true,true));
            takeover(&mut s);assert!(!s.open);assert!(s.pad_block);
            assert!(pad_step(&mut s,button,false,false,true));assert!(!s.open);
            assert!(pad_step(&mut s,0,false,true,true));
            assert!(!pad_step(&mut s,1,false,false,true));
        }
    }
    #[test]fn takeover_outside_modal_does_not_swallow_normal_input(){
        let mut s=Input::default();takeover(&mut s);
        assert!(!pad_step(&mut s,1,false,false,true));
    }
    #[test]fn detected_family_without_pilot_owner(){
        assert_eq!(preferred_family(0,0),0);
        assert_eq!(preferred_family(0,PS|((1<<PS)<<2)),PS);
        assert_eq!(preferred_family(XBOX,PS|(((1<<PS)|(1<<XBOX))<<2)),XBOX);
    }
    #[test]fn cursor_click_opens_consumes_release_then_closes(){
        let mut s=Input::default();
        assert!(!mouse_step(&mut s,0x80,false,false));assert!(!s.open);
        assert!(mouse_step(&mut s,0x80,false,true));assert!(s.open);
        assert!(mouse_step(&mut s,0x80,true,false));assert!(s.open);
        assert!(mouse_step(&mut s,0,true,false));assert!(s.open);
        assert!(mouse_step(&mut s,0x80,false,false));assert!(!s.open);
        assert!(mouse_step(&mut s,0,false,false));
        assert!(!mouse_step(&mut s,0,false,false));
    }
    #[test]fn invite_hit_matches_visible_bounds(){
        assert!(invite_hit(450,534));assert!(!invite_hit(450,510));
        assert!(!invite_hit(299,534));assert!(!invite_hit(606,534));
        assert!(!invite_hit(450,552));
    }
    #[test]fn page_exit_clears_captured_input_without_inventing_a_pad_edge(){
        let mut s=Input::default();
        assert!(mouse_step(&mut s,0x80,false,true));
        s.keys[30]=true;s.buttons=8;s.axis=true;
        leave_controls(&mut s);
        assert!(!s.open&&!s.mouse_block&&!s.pad_block);
        assert!(s.keys.iter().all(|&k|!k));
        assert_eq!(s.buttons,8);assert!(s.axis);
        assert!(!pad_step(&mut s,8,true,false,true));
        assert!(!s.open);
    }
    #[test]fn stale_modal_expires_even_without_active_gamepad(){
        let mut s=Input::default();
        assert!(mouse_step(&mut s,0x80,false,true));assert!(s.open);
        let t=s.present.unwrap();
        expire_preview(&mut s,t+Duration::from_secs(1));assert!(s.open);
        expire_preview(&mut s,t+Duration::from_secs(3));assert!(!s.open);
        assert!(s.mouse_block); // release remains consumed
        assert!(mouse_step(&mut s,0,false,false));
        assert!(!s.mouse_block);
    }
    #[test]fn rectangle_is_native_4by3(){assert_eq!(rect(0,0,800,600),Rect{left:0,top:0,right:800,bottom:600});}
}
