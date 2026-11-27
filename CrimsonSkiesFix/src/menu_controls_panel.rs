//! First in-game controls diagram: safe, non-modal hold-to-preview.
//! Only the exact observed Controls Preferences page; original UI remains.
use std::{ffi::c_void,ptr,sync::{Mutex,OnceLock,atomic::{AtomicBool,AtomicU8,Ordering}},time::{Duration,Instant}};
use crate::borderless_native_blt::Rect;
type H=*mut c_void;
const NONE:u8=0;
const CANDIDATE:u8=1;
const CONTROLS:u8=2;
static PAGE:AtomicU8=AtomicU8::new(NONE);
static PAD:OnceLock<Mutex<Option<Instant>>>=OnceLock::new();
static DRAW_KIND:AtomicU8=AtomicU8::new(0);
static PENDING:AtomicU8=AtomicU8::new(0);
static LAST_PRESENT:AtomicU8=AtomicU8::new(0);
static SEEN:AtomicBool=AtomicBool::new(false);
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
        if after!=CONTROLS{note_pad(false);}
        if after==CONTROLS&&crate::cfg().log_enabled{
            crate::log("MENU-CONTROLS-PANEL: verified cp_background + cp_t_joysticktitle; preview available");
        }
    }
}
pub fn clear_page(){
    if PAGE.swap(NONE,Ordering::AcqRel)!=NONE{note_pad(false);}
    not_presented();
}
pub fn page_active()->bool{PAGE.load(Ordering::Acquire)==CONTROLS}
pub fn note_pad(held:bool){
    if let Some(state)=PAD.get(){
        if let Ok(mut state)=state.try_lock(){*state=held.then(Instant::now);}
    }else if held{
        let _=PAD.set(Mutex::new(Some(Instant::now())));
    }
}
fn pad_held()->bool{
    PAD.get().and_then(|s|s.try_lock().ok()).and_then(|s|*s)
        .is_some_and(|t|t.elapsed()<Duration::from_millis(150))
}
#[link(name="user32")]
extern "system"{
    fn GetForegroundWindow()->H;
    fn GetWindowThreadProcessId(window:H,pid:*mut u32)->u32;
    fn GetAsyncKeyState(key:i32)->i16;
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
    fn LineTo(dc:H,x:i32,y:i32)->i32;
}
#[repr(C)]#[derive(Clone,Copy)]struct Point{x:i32,y:i32}
fn rect(l:i32,t:i32,r:i32,b:i32)->Rect{Rect{left:l,top:t,right:r,bottom:b}}
fn rgb(r:u32,g:u32,b:u32)->u32{r|(g<<8)|(b<<16)}
const INK:u32=0x00263849;
const PAPER:u32=0x009dbcd1;
const PANEL:u32=0x0085a5bd;
const EDGE:u32=0x00485c72;
const DARK:u32=0x00364655;
const LIGHT:u32=0x00b9d1e5;
pub fn eligible(view:Rect)->bool{
    page_active()&&crate::menu_gos_event::preferences_transaction_active()&&
        crate::menu_video_probe::legacy_ui_active()&&view==rect(0,0,800,600)
}
unsafe fn foreground()->bool{
    let w=GetForegroundWindow();let mut pid=0;
    !w.is_null()&&GetWindowThreadProcessId(w,&mut pid)!=0&&pid==GetCurrentProcessId()
}
unsafe fn down(k:i32)->bool{GetAsyncKeyState(k)<0}
unsafe fn preview()->bool{
    foreground()&&!down(1)&&!down(2)&&!down(0x1b)&&!down(0x0d)&&(down(0x70)||pad_held())
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
    unsafe fn line(&mut self,pts:&[(i32,i32)]){
        if pts.len()<2{return;}
        let pen=CreatePen(0,1,EDGE);if pen.is_null(){self.ok=false;return;}
        let old=SelectObject(self.dc,pen);
        if old.is_null()||old as isize == -1{self.ok=false;}else{
            self.ok&=MoveToEx(self.dc,pts[0].0,pts[0].1,ptr::null_mut())!=0;
            for &(x,y) in &pts[1..]{self.ok&=LineTo(self.dc,x,y)!=0;}
            SelectObject(self.dc,old);
        }
        DeleteObject(pen);
    }
    unsafe fn text(&mut self,r:Rect,s:&str,size:i32,bold:bool,align:u32,color:u32){
        let face:Vec<u16>=if bold{"Georgia"}else{"Arial"}.encode_utf16().chain([0]).collect();
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
    unsafe fn legend(&mut self,x:i32,y:i32,w:i32,head:&str,body:&str,right:bool){
        let align=if right{2}else{0};
        self.text(rect(x,y,x+w,y+20),head,15,true,align,INK);
        self.text(rect(x,y+21,x+w,y+41),body,14,false,align,INK);
    }
    unsafe fn panel(&mut self){
        self.fill(rect(42,49,758,551),DARK);
        self.fill(rect(46,53,754,547),PAPER);
        self.fill(rect(53,60,747,540),EDGE);
        self.fill(rect(55,62,745,538),PAPER);
        for (x,y) in [(62,69),(738,69),(62,531),(738,531)]{self.circle(x,y,3,EDGE);}
        self.text(rect(75,73,725,111),"FLIGHT CONTROLS",28,true,1,INK);
        self.line(&[(90,115),(710,115)]);
        self.text(rect(75,120,725,142),"SDL3  /  STANDARD GAMEPAD",14,false,1,INK);
        // Original vector illustration, not a bundled game or vendor asset.
        self.rounded(rect(275,196,341,230),PANEL,12);
        self.rounded(rect(459,196,525,230),PANEL,12);
        self.rounded(rect(268,220,344,242),EDGE,8);
        self.rounded(rect(456,220,532,242),EDGE,8);
        self.poly(&[(285,238),(310,228),(350,238),(450,238),(490,228),(515,238),
          (540,273),(553,333),(550,370),(536,394),(518,398),(496,376),(465,344),
          (335,344),(304,376),(282,398),(264,394),(250,370),(247,333),(260,273)],PANEL);
        self.circle(321,267,28,DARK);self.circle(321,267,20,EDGE);
        self.circle(454,327,28,DARK);self.circle(454,327,20,EDGE);
        self.poly(&[(338,298),(350,298),(350,311),(363,311),(363,323),(350,323),
          (350,336),(338,336),(338,323),(325,323),(325,311),(338,311)],DARK);
        self.rounded(rect(373,268,391,279),DARK,5);
        self.rounded(rect(409,268,427,279),DARK,5);
        self.circle(400,248,11,LIGHT);
        for (x,y,t) in [(487,251,"Y"),(509,273,"B"),(465,273,"X"),(487,295,"A")]{
            self.circle(x,y,10,DARK);
            self.text(rect(x-9,y-9,x+9,y+9),t,11,true,1,LIGHT);
        }
        self.legend(72,152,175,"LT / L2","Fire rockets",false);
        self.legend(72,202,175,"LB / L1","Previous enemy",false);
        self.legend(72,252,175,"LEFT STICK","Pitch / Roll",false);
        self.legend(72,302,175,"L3","Cycle rockets",false);
        self.legend(72,352,175,"D-PAD","Menu cursor",false);
        self.legend(553,152,175,"RT / R2","Fire guns",true);
        self.legend(553,202,175,"RB / R1","Next enemy",true);
        self.legend(553,252,175,"Y / Triangle","Target attacker",true);
        self.legend(553,302,175,"B / Circle","Nitro",true);
        self.legend(553,352,175,"A / Cross","Cycle guns",true);
        self.line(&[(205,173),(259,173),(300,196)]);
        self.line(&[(205,223),(254,223),(276,230)]);
        self.line(&[(205,273),(293,267)]);
        self.line(&[(205,323),(249,323),(301,283)]);
        self.line(&[(205,373),(302,373),(344,337)]);
        self.line(&[(595,173),(541,173),(500,196)]);
        self.line(&[(595,223),(546,223),(524,230)]);
        self.line(&[(595,270),(539,270),(497,251)]);
        self.line(&[(595,320),(564,320),(519,273)]);
        self.line(&[(595,369),(559,369),(497,295)]);
        self.line(&[(465,273),(445,254),(445,163),(380,163)]);
        self.text(rect(317,146,449,162),"X / Square: View",13,false,1,INK);
        self.line(&[(454,356),(454,399)]);
        self.text(rect(260,400,540,421),"RIGHT STICK: Yaw / Throttle",16,true,1,INK);
        self.text(rect(260,422,540,441),"Up: increase / Down: decrease",14,false,1,INK);
        self.text(rect(72,402,245,439),"Start / Options: Pause",14,true,0,INK);
        self.fill(rect(73,451,727,510),PANEL);
        self.text(rect(78,456,722,473),"MENUS: A / Cross = Select    B / Circle = Back",14,false,1,INK);
        self.text(rect(78,474,722,491),"CINEMATICS: A / B / Start = Skip",14,false,1,INK);
        self.text(rect(78,492,722,509),"Release Y / Triangle or F1 to return",14,true,1,INK);
        self.text(rect(80,515,720,534),"CrimsonSkiesFix",12,true,1,EDGE);
    }
}
pub unsafe fn draw(dc:H,view:Rect)->bool{
    if dc.is_null()||!eligible(view){return false;}
    let saved=SaveDC(dc);if saved==0{return false;}
    let shown=preview();
    let mut p=Painter{dc,ok:true};
    if shown{p.panel();}
    else{
        // Provisional location; original interactive controls remain visible.
        p.fill(rect(16,12,270,39),DARK);
        p.fill(rect(18,14,268,37),PAPER);
        p.text(rect(22,14,264,37),"Hold Y / Triangle or F1: Gamepad",13,false,1,INK);
    }
    let restored=RestoreDC(dc,saved)!=0;
    if p.ok&&restored{DRAW_KIND.store(if shown{2}else{1},Ordering::Release);true}else{false}
}
pub fn prepared(){PENDING.store(DRAW_KIND.load(Ordering::Acquire),Ordering::Release);}
pub fn not_presented(){PENDING.store(0,Ordering::Release);DRAW_KIND.store(0,Ordering::Release);LAST_PRESENT.store(0,Ordering::Release);}
pub fn note_presented(){
    let kind=PENDING.swap(0,Ordering::AcqRel);
    if kind==0{return;}
    let old=LAST_PRESENT.swap(kind,Ordering::AcqRel);
    if kind==2&&old!=2&&crate::cfg().log_enabled{
        // Bound diagnostic volume; first successful full panel presentation.
        if !SEEN.swap(true,Ordering::AcqRel){crate::log("MENU-CONTROLS-PANEL: static 4:3 diagram presented on private scratch");}
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
    #[test]fn rectangle_is_native_4by3(){assert_eq!(rect(0,0,800,600),Rect{left:0,top:0,right:800,bottom:600});}
}
