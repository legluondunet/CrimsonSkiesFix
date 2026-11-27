//! Modal static controls diagram, with active SDL3 controller artwork.
//! Only the exact observed Controls Preferences page; original UI remains.
use std::{ffi::c_void,ptr,sync::{Mutex,OnceLock,atomic::{AtomicBool,AtomicU8,Ordering}},time::{Duration,Instant}};
use crate::borderless_native_blt::Rect;
type H=*mut c_void;
const NONE:u8=0;
const CANDIDATE:u8=1;
const CONTROLS:u8=2;
static PAGE:AtomicU8=AtomicU8::new(NONE);
static FAMILY:AtomicU8=AtomicU8::new(0);
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
fn close(s:&mut Input){
    if s.open{
        s.open=false;s.present=None;
        if crate::cfg().log_enabled{crate::log("MENU-CONTROLS-PANEL: closed");}
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
    if let Ok(mut s)=input().lock(){close(&mut s);s.buttons=0;s.axis=false;s.pad_block=false;}
    if crate::cfg().log_enabled{crate::log(&format!("MENU-CONTROLS-PANEL: active family={family} SDL type={kind}"));}
}
pub fn device_removed(){
    FAMILY.store(0,Ordering::Release);
    reset_input();
}
pub fn reset_input(){
    if let Ok(mut s)=input().lock(){close(&mut s);*s=Input::default();}
}
fn context()->bool{page_active()&&crate::menu_gos_event::preferences_transaction_active()&&crate::menu_video_probe::legacy_ui_active()}
fn pad_step(s:&mut Input,buttons:u32,axis:bool,neutral:bool,eligible:bool)->bool{
    let pressed=buttons&!s.buttons;
    let moved=axis&&!s.axis;
    s.buttons=buttons;s.axis=axis;
    if !eligible{close(s);s.pad_block=false;return false;}
    if s.open{
        if pressed!=0||moved{close(s);s.pad_block=true;}
        return true;
    }
    if s.pad_block{
        if neutral{s.pad_block=false;}
        return true;
    }
    if pressed==OPEN_BUTTON&&buttons==OPEN_BUTTON&&!axis{
        s.open=true;s.pad_block=true;s.present=Some(Instant::now());
        if crate::cfg().log_enabled{crate::log("MENU-CONTROLS-PANEL: opened by active gamepad");}
        return true;
    }
    false
}
pub fn pad_event(buttons:u32,axis:bool,neutral:bool,eligible:bool)->bool{
    let Ok(mut s)=input().lock()else{return false;};
    if s.open&&s.present.is_some_and(|t|t.elapsed()>Duration::from_secs(2)){close(&mut s);}
    pad_step(&mut s,buttons,axis,neutral,eligible&&context()&&FAMILY.load(Ordering::Acquire)!=0)
}
pub fn mouse_event(buttons:u32,moved:bool)->bool{
    if !context(){return false;}
    let Ok(mut s)=input().lock()else{return false;};
    if s.open{
        if buttons!=0||moved{close(&mut s);s.mouse_block=buttons!=0;}
        return true;
    }
    if s.mouse_block{
        if buttons==0{s.mouse_block=false;}
        return true;
    }
    false
}
pub fn keyboard_capture()->bool{
    context()&&input().lock().is_ok_and(|s|s.open||s.keys.iter().any(|&v|v))
}
pub fn keyboard_event(scan:usize,pressed:bool){
    if scan>=256{return;}
    if let Ok(mut s)=input().lock(){
        s.keys[scan]=pressed;
        if pressed{close(&mut s);}
    }
}
pub fn is_open()->bool{context()&&input().lock().is_ok_and(|s|s.open)}
pub fn fail_preview(){if let Ok(mut s)=input().lock(){close(&mut s);}}
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
        if after!=CONTROLS{fail_preview();}
        if after==CONTROLS&&crate::cfg().log_enabled{
            crate::log("MENU-CONTROLS-PANEL: verified cp_background + cp_t_joysticktitle; preview available");
        }
    }
}
pub fn clear_page(){
    if PAGE.swap(NONE,Ordering::AcqRel)!=NONE{fail_preview();}
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
    context()&&FAMILY.load(Ordering::Acquire)!=0&&view==rect(0,0,800,600)
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
    unsafe fn panel(&mut self,family:u8){
        let b=labels(family);
        self.fill(rect(42,49,758,551),DARK);
        self.fill(rect(46,53,754,547),PAPER);
        self.fill(rect(53,60,747,540),EDGE);
        self.fill(rect(55,62,745,538),PAPER);
        for (x,y) in [(62,69),(738,69),(62,531),(738,531)]{self.circle(x,y,3,EDGE);}
        self.text(rect(75,73,725,111),tr("FLIGHT CONTROLS","COMMANDES DE VOL","FLUGSTEUERUNG"),28,true,1,INK);
        self.line(&[(90,115),(710,115)]);
        self.text(rect(75,120,725,142),match family{PS=>"PLAYSTATION",XBOX=>"XBOX",_=>tr("STANDARD GAMEPAD","MANETTE STANDARD","STANDARD-GAMEPAD")},14,false,1,INK);
        // Original vectors; independent geometry for symmetric PlayStation
        // sticks and offset Xbox sticks. Unidentified pads use positional labels.
        self.rounded(rect(275,192,341,226),PANEL,12);
        self.rounded(rect(459,192,525,226),PANEL,12);
        self.rounded(rect(268,216,344,239),EDGE,8);
        self.rounded(rect(456,216,532,239),EDGE,8);
        let (lx,ly,dx,dy,rx,ry)=if family==PS{
            self.poly(&[(285,238),(310,229),(350,235),(450,235),(490,229),(515,238),
                (535,262),(546,304),(550,351),(546,378),(533,395),(514,398),(496,386),(470,351),
                (330,351),(304,386),(286,398),(267,395),(254,378),(250,351),(254,304),(265,262)],PANEL);
            self.rounded(rect(354,237,446,285),EDGE,8);
            self.rounded(rect(337,249,348,261),DARK,5);
            self.rounded(rect(452,249,463,261),DARK,5);
            self.circle(400,317,10,LIGHT);
            (346,327,315,267,454,327)
        }else{
            self.poly(&[(285,238),(310,228),(350,238),(450,238),(490,228),(515,238),
                (540,273),(553,333),(550,370),(536,394),(518,398),(496,376),(465,344),
                (335,344),(304,376),(282,398),(264,394),(250,370),(247,333),(260,273)],PANEL);
            self.rounded(rect(373,268,391,279),DARK,5);
            self.rounded(rect(409,268,427,279),DARK,5);
            self.circle(400,248,11,LIGHT);
            (321,267,344,317,454,327)
        };
        for (x,y) in [(lx,ly),(rx,ry)]{self.circle(x,y,28,DARK);self.circle(x,y,20,EDGE);}
        self.poly(&[(dx-6,dy-19),(dx+6,dy-19),(dx+6,dy-6),(dx+19,dy-6),
            (dx+19,dy+6),(dx+6,dy+6),(dx+6,dy+19),(dx-6,dy+19),
            (dx-6,dy+6),(dx-19,dy+6),(dx-19,dy-6),(dx-6,dy-6)],DARK);
        for ((x,y),t) in [(487,251),(509,273),(465,273),(487,295)].into_iter().zip(b.face){
            self.circle(x,y,10,DARK);
            // Draw the PS glyphs as geometry so no symbol font is required.
            if family==PS{
                match t{
                    "△"=>self.poly(&[(x,y-5),(x-5,y+4),(x+5,y+4)],LIGHT),
                    "○"=>{self.circle(x,y,5,LIGHT);self.circle(x,y,3,DARK);},
                    "□"=>self.rounded(rect(x-5,y-5,x+5,y+5),LIGHT,1),
                    _=>{self.line(&[(x-5,y-5),(x+5,y+5)]);self.line(&[(x+5,y-5),(x-5,y+5)]);}
                }
            }else{self.text(rect(x-9,y-9,x+9,y+9),t,11,true,1,LIGHT);}
        }
        self.legend(72,152,175,b.lt,tr("Fire rockets","Tirer les roquettes","Raketen abfeuern"),false);
        self.legend(72,202,175,b.lb,tr("Previous enemy","Ennemi précédent","Vorheriger Gegner"),false);
        self.legend(72,252,175,tr("LEFT STICK","STICK GAUCHE","LINKER STICK"),tr("Pitch / Roll","Tangage / Roulis","Nicken / Rollen"),false);
        self.legend(72,302,175,if family==PS{"L3"}else{tr("L3 / LS","L3 / LS","L3 / LS")},tr("Cycle rockets","Changer de roquettes","Raketen wechseln"),false);
        self.legend(72,352,175,tr("D-PAD","CROIX DIRECTIONNELLE","STEUERKREUZ"),tr("Menu cursor","Curseur des menus","Menüzeiger"),false);
        self.legend(553,146,175,b.rt,tr("Fire guns","Tirer aux mitrailleuses","MG abfeuern"),true);
        self.legend(553,190,175,b.rb,tr("Next enemy","Ennemi suivant","Nächster Gegner"),true);
        self.legend(553,234,175,b.north,tr("Target attacker","Cibler l'attaquant","Angreifer anvisieren"),true);
        self.legend(553,278,175,b.east,tr("Nitro","Nitro","Nitro"),true);
        self.legend(553,322,175,b.south,tr("Cycle guns","Changer d'arme","Waffen wechseln"),true);
        self.legend(553,366,175,b.west,tr("Cycle views","Changer de vue","Ansicht wechseln"),true);
        self.line(&[(205,173),(259,173),(300,192)]);
        self.line(&[(205,223),(254,223),(276,226)]);
        self.line(&[(205,273),(lx-28,ly)]);
        self.line(&[(205,323),(249,323),(lx-19,ly+20)]);
        self.line(&[(205,373),(dx-20,373),(dx,dy+19)]);
        self.line(&[(595,167),(541,167),(500,192)]);
        self.line(&[(595,211),(546,211),(524,226)]);
        self.line(&[(595,255),(539,255),(497,251)]);
        self.line(&[(595,299),(564,299),(519,273)]);
        self.line(&[(595,343),(559,343),(497,295)]);
        self.line(&[(465,283),(477,308),(482,371),(595,387)]);
        self.line(&[(rx,ry+28),(rx,399)]);
        self.text(rect(260,400,540,421),tr("RIGHT STICK: Yaw / Throttle","STICK DROIT : Lacet / Gaz","RECHTER STICK: Gieren / Gas"),15,true,1,INK);
        self.text(rect(260,422,540,441),tr("Up: increase / Down: decrease","Haut : augmenter / Bas : réduire","Oben: mehr / Unten: weniger"),14,false,1,INK);
        let pause=format!("{} : {}",b.start,tr("Pause","Pause","Pause"));
        self.text(rect(72,402,245,439),&pause,14,true,0,INK);
        self.fill(rect(73,451,727,510),PANEL);
        let menus=format!("{}: {} = {}     {} = {}",tr("MENUS","MENUS","MENÜS"),b.south,tr("Select","Valider","Auswählen"),b.east,tr("Back","Retour","Zurück"));
        let movies=format!("{}: {} / {} / {} = {}",tr("CINEMATICS","CINÉMATIQUES","ZWISCHENSEQUENZEN"),b.south,b.east,b.start,tr("Skip","Passer","Überspringen"));
        self.text(rect(78,456,722,473),&menus,14,false,1,INK);
        self.text(rect(78,474,722,491),&movies,14,false,1,INK);
        self.text(rect(78,492,722,509),tr("Press any button or key to return","Une touche ou un bouton pour revenir","Beliebige Taste zum Schließen"),14,true,1,INK);
        self.text(rect(80,515,720,534),"CrimsonSkiesFix",12,true,1,EDGE);
    }
}
pub unsafe fn draw(dc:H,view:Rect)->bool{
    if dc.is_null()||!eligible(view){return false;}
    let saved=SaveDC(dc);if saved==0{return false;}
    if !foreground(){fail_preview();}
    let shown=is_open();
    let family=FAMILY.load(Ordering::Acquire);
    let mut p=Painter{dc,ok:true};
    if shown{p.panel(family);}
    else{
        // Below the native Apply/Reset bar, not across the game logo.
        let b=labels(family);
        let hint=format!("{} : {}",b.north,tr("Gamepad controls","Commandes manette","Gamepad-Steuerung"));
        p.fill(rect(230,534,675,560),DARK);
        p.fill(rect(232,536,673,558),EDGE);
        p.text(rect(237,536,668,558),&hint,15,true,1,LIGHT);
    }
    let restored=RestoreDC(dc,saved)!=0;
    if p.ok&&restored{DRAW_KIND.store(if shown{2}else{1},Ordering::Release);true}else{fail_preview();false}
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
    #[test]fn rectangle_is_native_4by3(){assert_eq!(rect(0,0,800,600),Rect{left:0,top:0,right:800,bottom:600});}
}
