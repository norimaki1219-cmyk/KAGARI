use super::*;
pub const SYMBOLS: [&str; 4] = ["○", "▶", "◷", "✓"];
const ACTIONS: &[(i32, u16, u32)] = &[
    (185, 0xe8b7, 0xf6e9d3),
    (186, 0xe721, 0xf6e9d3),
    (187, 0xe8ab, 0xf6e9d3),
    (188, 0xe8bd, 0xf6e9d3),
    (193, 0xe89f, 0x51402d),
    (190, 0xe81c, 0xf6e9d3),
    (189, 0xe756, 0xf6e9d3),
    (127, 0xe710, 0x806014),
    (143, 0xe72c, 0x806014),
    (126, 0xe71c, 0x805322),
    (128, 0xe894, 0x756759),
    (119, 0xe72c, 0x805322),
    (159, 0xe718, 0x805322),
    (113, 0xe710, 0x806014),
];
pub fn action(id: i32) -> bool {
    ACTIONS.iter().any(|a| a.0 == id)
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    r: i32,
    b: i32,
}
#[repr(C)]
struct Draw {
    kind: u32,
    id: u32,
    index: u32,
    action: u32,
    state: u32,
    hwnd: H,
    dc: H,
    rect: Rect,
    data: usize,
}
#[repr(C)]
struct Tool {
    size: u32,
    flags: u32,
    hwnd: H,
    id: usize,
    rect: Rect,
    instance: H,
    text: isize,
    param: H,
    reserved: H,
}
#[repr(C)]
struct Tip {
    header: native::Header,
    text: usize,
    buffer: [u16; 80],
    instance: H,
    flags: u32,
    param: H,
}
#[repr(C)]
struct Custom {
    header: native::Header,
    stage: u32,
    dc: H,
    rect: Rect,
    item: usize,
    state: u32,
    param: H,
}
#[repr(C)]
struct ListDraw {
    base: Custom,
    color: u32,
    bg: u32,
    sub: i32,
}
#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex: u32,
        c: Ptr,
        n: Ptr,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: H,
        menu: H,
        instance: H,
        param: H,
    ) -> H;
    fn DrawTextW(dc: H, s: Ptr, len: i32, r: *mut Rect, flags: u32) -> i32;
    fn FillRect(dc: H, r: &Rect, b: H) -> i32;
    fn DrawFocusRect(dc: H, r: &Rect) -> i32;
    fn SetWindowLongPtrW(h: H, id: i32, v: H) -> H;
    fn GetDlgCtrlID(h: H) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        h: i32,
        w: i32,
        e: i32,
        o: i32,
        weight: i32,
        italic: u32,
        under: u32,
        strike: u32,
        cs: u32,
        out: u32,
        clip: u32,
        q: u32,
        pitch: u32,
        face: Ptr,
    ) -> H;
    fn CreateSolidBrush(c: u32) -> H;
    fn SelectObject(dc: H, o: H) -> H;
    fn DeleteObject(o: H) -> i32;
    fn SetTextColor(dc: H, c: u32) -> u32;
    fn SetBkMode(dc: H, mode: i32) -> i32;
}
pub(super) struct DecisionFont(H);
impl Drop for DecisionFont {
    fn drop(&mut self){unsafe{DeleteObject(self.0);}}
}
pub(super) fn decision_font(dialog:H)->Option<DecisionFont>{
    #[link(name="user32")]
    extern "system"{fn GetDpiForWindow(h:H)->u32;}
    let dpi=unsafe{GetDpiForWindow(dialog)}.max(96) as i32;
    let font=unsafe{CreateFontW(-((14*dpi+36)/72),0,0,0,400,0,0,0,1,0,0,5,0,wide("Yu Gothic UI").as_ptr())};
    if font==0{return None;}
    for id in [403,421,439,681,683,684,685,686]{msg(dialog,id,0x30,font as usize,1);}
    for id in [681,683]{msg(dialog,id,0x1a0,0,((28*dpi+48)/96) as isize);}
    Some(DecisionFont(font))
}
pub(super) fn fit_list_width<'a>(dialog:H,id:i32,items:impl Iterator<Item=&'a str>){
    #[link(name="user32")]
    extern "system"{fn GetDC(h:H)->H;fn ReleaseDC(h:H,dc:H)->i32;}
    #[link(name="gdi32")]
    extern "system"{fn GetTextExtentPoint32W(dc:H,text:Ptr,len:i32,size:*mut [i32;2])->i32;}
    unsafe{
        let list=GetDlgItem(dialog,id);
        let dc=GetDC(list);
        if dc==0{return;}
        let old=SelectObject(dc,msg(dialog,id,0x31,0,0));
        let mut width=0;
        for item in items{
            let text=wide(item);
            let mut size=[0;2];
            if GetTextExtentPoint32W(dc,text.as_ptr(),(text.len()-1) as i32,&mut size)!=0{width=width.max(size[0]+8);}
        }
        SelectObject(dc,old);ReleaseDC(list,dc);
        msg(dialog,id,0x194,width as usize,0);
    }
}
pub(super) fn draw_choice(h:H,l:isize){
    #[link(name="user32")]
    extern "system"{fn GetSysColorBrush(index:i32)->H; fn GetSysColor(index:i32)->u32;}
    let d=unsafe{&*(l as *const Draw)};
    let selected=d.state & 1 != 0;
    unsafe {
        FillRect(d.dc,&d.rect,GetSysColorBrush(if selected {13}else{5}));
        if d.index!=u32::MAX {
            let index=d.index as usize;
            let len=msg(h,683,0x18a,index,0);
            if len>=0 {
                let mut text=vec![0u16;len as usize+1];
                msg(h,683,0x189,index,text.as_mut_ptr() as isize);
                let mut rect=d.rect;
                rect.x+=3;
                SetBkMode(d.dc,1);
                SetTextColor(d.dc,GetSysColor(if selected {14}else{8}));
                DrawTextW(d.dc,text.as_ptr(),len as i32,&mut rect,0x800);
            }
        }
        if d.state & 16 != 0 {DrawFocusRect(d.dc,&d.rect);}
    }
}
pub fn init(h: H) {
    unsafe {
        let tip = CreateWindowExW(
            8,
            wide("tooltips_class32").as_ptr(),
            std::ptr::null(),
            0x80000003,
            0,
            0,
            0,
            0,
            h,
            0,
            GetModuleHandleW(std::ptr::null()),
            0,
        );
        for &(id, _, _) in ACTIONS {
            let tool = Tool {
                size: std::mem::size_of::<Tool>() as _,
                flags: 0x11,
                hwnd: h,
                id: GetDlgItem(h, id) as usize,
                rect: Rect::default(),
                instance: 0,
                text: -1,
                param: 0,
                reserved: 0,
            };
            SendMessageW(tip, 0x432, 0, &tool as *const _ as _);
        }
    }
}
pub fn hint(l: isize) {
    unsafe {
        let t = &mut *(l as *mut Tip);
        GetWindowTextW(t.header.id as H, t.buffer.as_mut_ptr(), 80);
        t.text = t.buffer.as_ptr() as _;
    }
}
pub fn draw(l: isize) {
    unsafe {
        let d = &*(l as *const Draw);
        let Some(&(_, mut glyph, color)) = ACTIONS.iter().find(|a| a.0 == d.id as i32) else {
            return;
        };
        let active = d.id == 159 && GetWindowTextLengthW(d.hwnd) > 0 && {
            let mut b = [0u16; 2];
            GetWindowTextW(d.hwnd, b.as_mut_ptr(), 2);
            b[0] == '★' as u16
        };
        if active {
            glyph = 0xe77a;
        }
        let brush = CreateSolidBrush(if (185..=190).contains(&d.id) {
            if d.state & 1 != 0 {
                0x796132
            } else {
                0x51402d
            }
        } else if d.state & 1 != 0 || active {
            0xeadcca
        } else {
            0xf6f4f0
        });
        FillRect(d.dc, &d.rect, brush);
        DeleteObject(brush);
        let font = CreateFontW(
            -((d.rect.b - d.rect.y) * 57 / 100).max(12),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe MDL2 Assets").as_ptr(),
        );
        let old = SelectObject(d.dc, font);
        SetBkMode(d.dc, 1);
        SetTextColor(d.dc, if d.state & 4 != 0 { 0xa89f96 } else { color });
        let mut r = d.rect;
        DrawTextW(d.dc, &glyph, 1, &mut r, 0x25);
        SelectObject(d.dc, old);
        DeleteObject(font);
        if d.state & 16 != 0 {
            DrawFocusRect(d.dc, &d.rect);
        }
    }
}
#[link(name="user32")]
extern "system" {
    fn LoadImageW(instance:H, name:Ptr, kind:u32, width:i32, height:i32, flags:u32)->H;
    fn DrawIconEx(dc:H,x:i32,y:i32,icon:H,w:i32,h:i32,step:u32,brush:H,flags:u32)->i32;
    fn GetDpiForWindow(h:H)->u32;
}
fn action_icon(hwnd:H,resource:usize)->(H,i32) {
    let size=(unsafe{GetDpiForWindow(hwnd)}.max(96) as i32*16+48)/96;
    let icon=unsafe{LoadImageW(GetModuleHandleW(std::ptr::null()),resource as Ptr,1,size,size,0x8000)};
    (icon,size)
}
pub(super) fn set_action_icon(h:H,id:i32,resource:usize){
    let (icon,_)=action_icon(h,resource);
    msg(h,id,0xF7,1,icon); // BM_SETIMAGE with the shared icon and the existing button label.
}
pub fn draw_send(h:H,l:isize){
    let d=unsafe{&*(l as *const Draw)};
    let disabled=d.state&4!=0;
    let label=get(h,404);
    let running=label=="実行中…";
    let fill=if disabled && running {0xe4e8e8}else if disabled {0x676752}else if d.state&1!=0{0x404605}else{0x5f6508};
    unsafe{
        let brush=CreateSolidBrush(fill);FillRect(d.dc,&d.rect,brush);DeleteObject(brush);
        let old=SelectObject(d.dc,SendMessageW(d.hwnd,0x31,0,0));
        SetBkMode(d.dc,1);SetTextColor(d.dc,if disabled && running{0x636666}else{0xffffff});
        let mut rect=d.rect;
        let resource=match label.as_str(){"実行中…"=>33,"再送する"=>32,_=>31};
        let (icon,size)=action_icon(d.hwnd,resource);
        let text=wide(&label);
        let mut bounds=Rect::default();
        DrawTextW(d.dc,text.as_ptr(),-1,&mut bounds,0x420);
        let gap=(size/4).max(2);
        let x=(rect.x+(rect.r-rect.x-size-gap-bounds.r)/2).max(rect.x);
        DrawIconEx(d.dc,x,(rect.y+rect.b-size)/2,icon,size,size,0,0,3);
        rect.x=x+size+gap;
        DrawTextW(d.dc,text.as_ptr(),-1,&mut rect,0x24);
        if d.state&16!=0 {DrawFocusRect(d.dc,&rect);}
        if old!=0{SelectObject(d.dc,old);}
    }
}
// COLORREF values use BGR byte order.
#[link(name="user32")]
extern "system" { fn GetSysColorBrush(index:i32)->H; }
pub fn task_caption(h: H, dc: usize, hwnd: H) -> H {
    if unsafe{GetDlgCtrlID(hwnd)}==433 {
        let status=get(h,433);
        let (ink,fill)=if status.starts_with("実行中"){(0x745211,0xf5eee0)}
            else if status.starts_with("回答受信"){(0x3e5920,0xe6f2e1)}
            else if status=="未依頼"{(0x505a54,0xeaf0ed)}
            else{(0x1f3d80,0xdbebfa)};
        #[link(name="gdi32")]
        extern "system"{fn SetDCBrushColor(dc:H,c:u32)->u32;fn GetStockObject(id:i32)->H;fn SetBkColor(dc:H,c:u32)->u32;}
        unsafe{SetTextColor(dc as H,ink);SetBkColor(dc as H,fill);SetDCBrushColor(dc as H,fill);return GetStockObject(18);}
    }
    let color = match unsafe { GetDlgCtrlID(hwnd) } {
        420 => 0x775221, // summary, blue
        421 => 0x626018, // next action, teal
        431 | 432 => 0x734366, // history, purple
        _ => return 0,
    };
    unsafe {
        SetTextColor(dc as H, color);
        SetBkMode(dc as H, 1);
        GetSysColorBrush(15)
    }
}
pub fn legend(dc: usize, hwnd: H) -> H {
    let brush = native::background(dc);
    let id = unsafe { GetDlgCtrlID(hwnd) };
    if (170..=174).contains(&id) {
        unsafe {
            SetTextColor(dc as H, chart::COLORS[(id - 170) as usize]);
        }
    }
    brush
}
pub fn list_draw(h: H, l: isize) -> isize {
    unsafe {
        let d = &mut *(l as *mut ListDraw);
        let result = match d.base.stage {
            0x30001 if d.sub == 3 => {
                brand::list(h, d.base.dc, d.base.item);
                4
            }
            1 | 0x10001 => 0x20,
            0x30001 if d.sub == 2 && d.base.state & 1 == 0 => {
                d.color = chart::COLORS[(d.base.param as usize).min(3)];
                0
            }
            _ => 0,
        };
        SetWindowLongPtrW(h, 0, result);
        1
    }
}
