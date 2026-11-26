use super::*;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    x: i32,
    y: i32,
    r: i32,
    b: i32,
}
#[link(name = "comctl32")]
extern "system" {
    fn InitCommonControlsEx(x: *const [u32; 2]) -> i32;
}
#[link(name = "uxtheme")]
extern "system" {
    fn SetWindowTheme(h: H, s: Ptr, p: Ptr) -> i32;
}
#[link(name = "user32")]
extern "system" {
    fn GetDpiForWindow(h:H)->u32;
    fn GetClientRect(h: H, r: *mut Rect) -> i32;
    fn GetWindowRect(h: H, r: *mut Rect) -> i32;
    fn MapWindowPoints(from: H, to: H, p: *mut Rect, n: u32) -> i32;
    fn GetWindow(h: H, cmd: u32) -> H;
    fn MoveWindow(h: H, x: i32, y: i32, w: i32, height: i32, redraw: i32) -> i32;
    fn LoadIconW(h: H, i: Ptr) -> H;
}
#[repr(C)]
#[derive(Default)]
struct Column {
    mask: u32,
    fmt: i32,
    width: i32,
    text: usize,
    max: i32,
    sub: i32,
    image: i32,
    order: i32,
    min: i32,
    default_width: i32,
    ideal: i32,
}
#[repr(C)]
#[derive(Default)]
pub(super) struct Item {
    mask: u32,
    row: i32,
    col: i32,
    state: u32,
    state_mask: u32,
    text: usize,
    max: i32,
    image: i32,
    param: H,
    indent: i32,
    group: i32,
    cols: u32,
    column_ptr: usize,
    formats: usize,
    index: i32,
}
pub(super) fn brand_item(buf: *mut u16, n: usize) -> Item {
    Item {
        col: 3,
        text: buf as usize,
        max: n as i32,
        ..Default::default()
    }
}
type CornerProc = extern "system" fn(H,u32,usize,isize,usize,usize)->isize;
#[link(name="comctl32")]
extern "system" {
    fn SetWindowSubclass(h:H, proc:CornerProc, id:usize, data:usize)->i32;
    fn DefSubclassProc(h:H, message:u32, w:usize, l:isize)->isize;
}
#[link(name="user32")]
extern "system" {
    fn SetWindowRgn(h:H, region:H, redraw:i32)->i32;
    fn GetClassNameW(h:H, buffer:*mut u16, len:i32)->i32;
    fn GetWindowLongW(h:H,index:i32)->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn CreateRoundRectRgn(x:i32,y:i32,right:i32,bottom:i32,width:i32,height:i32)->H;
    fn DeleteObject(object:H)->i32;
}
fn round_control(h:H) {
    unsafe {
        let mut r=Rect::default();
        if GetWindowRect(h,&mut r)==0 || r.r<=r.x || r.b<=r.y {return;}
        let diameter=(12*GetDpiForWindow(h).max(96)/96) as i32;
        let region=CreateRoundRectRgn(0,0,r.r-r.x+1,r.b-r.y+1,diameter,diameter);
        if region!=0 && SetWindowRgn(h,region,1)==0 {DeleteObject(region);}
    }
}
extern "system" fn corner_proc(h:H,m:u32,w:usize,l:isize,_id:usize,_data:usize)->isize {
    let result=unsafe{DefSubclassProc(h,m,w,l)};
    if matches!(m,5|0x02e3) {round_control(h);}
    result
}
pub(super) fn round_fields(h:H) {
    unsafe {
        let mut child=GetWindow(h,5);
        while child!=0 {
            let mut name=[0u16;32];
            let count=GetClassNameW(child,name.as_mut_ptr(),32);
            let class=String::from_utf16_lossy(&name[..count as usize]).to_ascii_lowercase();
            let push=class=="button" && matches!(GetWindowLongW(child,-16)&15,0|1|11);
            if push || matches!(class.as_str(),"edit"|"combobox"|"richedit50w"|"richedit20w") {
                SetWindowSubclass(child,corner_proc,0x4b43,0);
                round_control(child);
            }
            child=GetWindow(child,2);
        }
    }
}
pub fn controls() {
    unsafe {
        InitCommonControlsEx(&[8, 35]);
    }
}
pub fn init(h: H) {
    visual::init(h);
    for (id, hint) in [
        (110, "YYYY-MM-DD"),
        (111, "例 15:00"),
        (182, "タスクを検索"),
        (123, "SHIRANUIログのパス"),
    ] {
        msg(h, id, 0x1501, 0, wide(hint).as_ptr() as _);
    }
    unsafe {
        let icon = LoadIconW(GetModuleHandleW(std::ptr::null()), 1 as Ptr);
        for size in 0..2 {
            SendMessageW(h, 0x80, size, icon);
        }
        SetWindowTheme(
            GetDlgItem(h, 106),
            wide("Explorer").as_ptr(),
            std::ptr::null(),
        );
    }
    msg(h, 106, 0x1036, 0x10020, 0x10020);
    for (i, title) in ["タスク名", "優先理由", "状態", "担当AI", "次の確認"]
        .iter()
        .enumerate()
    {
        let name = wide(title);
        let c = Column {
            mask: 7,
            text: name.as_ptr() as _,
            width: 90,
            ..Default::default()
        };
        msg(h, 106, 0x1061, i, &c as *const _ as _);
    }
    resize_columns(h);
}
fn resize_columns(h: H) {
    let mut r = Rect::default();
    unsafe {
        GetClientRect(GetDlgItem(h, 106), &mut r);
    }
    for (i, pc) in [40, 18, 13, 13, 16].iter().enumerate() {
        msg(h, 106, 0x101e, i, ((r.r - 18) * pc / 100) as _);
    }
}
pub fn row(h: H, i: usize, fields: [&str; 5], status: usize, selected: bool) {
    for (j, s) in fields.iter().enumerate() {
        let value = wide(s);
        let item = Item {
            mask: if j == 0 { 5 } else { 1 },
            param: status as H,
            row: i as _,
            col: j as _,
            text: value.as_ptr() as _,
            ..Default::default()
        };
        msg(
            h,
            106,
            if j == 0 { 0x104d } else { 0x1074 },
            i,
            &item as *const _ as _,
        );
    }
    if selected {
        let item = Item {
            state: 3,
            state_mask: 3,
            ..Default::default()
        };
        msg(h, 106, 0x102b, i, &item as *const _ as _);
    }
}
#[derive(Default)]
pub(super) struct Layout {
    pub(super) log_expanded: bool,
    pub(super) detail_view: bool,
    pub(super) settings_open: bool,
    pub(super) history_tab: bool,
    base: Rect,
    outer: Rect,
    children: Vec<(H, Rect)>,
}
thread_local! {static LAYOUT:RefCell<Layout>=RefCell::default();}
pub fn capture(h: H) {
    LAYOUT.with(|v| *v.borrow_mut() = Layout::capture(h));
}
impl Layout {
pub(super) fn capture(h: H) -> Self {
    round_fields(h);
    unsafe {
        let mut l = Layout::default();
        GetClientRect(h, &mut l.base);
        GetWindowRect(h, &mut l.outer);
        let mut c = GetWindow(h, 5);
        while c != 0 {
            let mut r = Rect::default();
            GetWindowRect(c, &mut r);
            MapWindowPoints(0, h, &mut r, 2);
            l.children.push((c, r));
            c = GetWindow(c, 2);
        }
        l
    }
}
    pub(super) fn minimum(&self, ptr: isize) {
        let r = self.outer;
        if r.r > r.x {
            unsafe {
                let p = ptr as *mut i32;
                *p.add(6) = if self.detail_view {let dpi=GetDpiForWindow(self.children.first().map_or(0,|c|c.0)).max(96); (700*dpi as i32/96).min(r.r-r.x)}else{r.r-r.x};
                *p.add(7) = r.b - r.y;
            }
        }
    }

    fn detail_layout(&self,h:H,now:Rect){
        if self.base.r==0 || self.base.b==0{return;}
        let sy=|v:i32|v*self.base.b/352;
        let sx=|v:i32|v*now.r/508;
        let place=|id,x,y,w,height:i32|unsafe{MoveWindow(GetDlgItem(h,id),sx(x),y,sx(w),(height as i32).max(1),1)};
        let show=|id,on|unsafe{ShowWindow(GetDlgItem(h,id),if on{5}else{0})};
        let compact=now.r*96 < unsafe{GetDpiForWindow(h)} as i32*1100;
        let top=sy(if self.log_expanded{60}else{31});
        place(447,10,sy(8),286,sy(16)); place(448,300,sy(5),97,sy(20)); place(416,401,sy(5),97,sy(20));
        place(433,10,sy(28),488,sy(29));
        place(450,10,top,104,sy(18));
        set(h,450,if self.settings_open{"設定を閉じる"}else{"タスク設定"});
        for &(child,r) in &self.children {
            let id=unsafe{GetDlgCtrlID(child)};
            if matches!(id,422..=429|107..=111|157|158|402|408|415) {
                show(id,self.settings_open);
                if self.settings_open {unsafe{MoveWindow(child,r.x*now.r/self.base.r,top+sy(21)+r.y-sy(67),(r.r-r.x)*now.r/self.base.r,if matches!(id,107|108|109|402|408|415){sy(100)}else{r.b-r.y},1);}}
            }
        }
        let heading=top+sy(if self.settings_open{114}else{22});
        let body=heading+sy(23);
        let bottom=now.b-sy(28);
        let attachments=has_preview(h);
        let preview=if attachments{sy(30)}else{0};
        let lines=(msg(h,403,0xba,0,0) as i32).clamp(2,5);
        let input_height=sy(16+9*lines).min(sy(65));
        let input=bottom-preview-sy(17)-input_height;
        let tools=input-sy(20);
        let hist=!compact || self.history_tab;
        show(451,compact);show(452,compact);show(431,!compact);show(432,!compact);
        show(409,!compact || !self.history_tab);show(405,hist);show(453,hist);
        place(451,10,heading,68,sy(19));place(452,82,heading,68,sy(19));
        place(431,10,heading,277,sy(18));place(432,306,heading,105,sy(18));place(453,414,heading,84,sy(19));
        place(409,10,body,if compact{488}else{286},tools-sy(4)-body);
        place(405,if compact{10}else{306},body,if compact{488}else{192},tools-sy(4)-body);
        place(411,10,tools,80,sy(18));place(412,93,tools,86,sy(18));place(406,183,tools,315,sy(18));
        place(403,10,input,488,input_height);
        place(444,10,input+input_height,488,preview);
        if attachments {unsafe{let c=GetDlgItem(h,444);let (w,b)=client_size(c);MoveWindow(GetWindow(c,5),0,0,w,b,1);}}
        place(413,10,bottom-sy(16),240,sy(13));place(434,253,bottom-sy(16),245,sy(15));
        for (id,x,w) in [(2,10,54),(437,69,90),(445,213,85),(114,303,95),(404,403,95)] {place(id,x,bottom,w,sy(20));}
    }

    pub(super) fn resize_task(&self, h: H, document: bool) {
        let mut now = Rect::default();
        unsafe { GetClientRect(h, &mut now); }
        if self.detail_view && !document {self.detail_layout(h,now);return;}
        let dx = now.r - self.base.r;
        let dy = now.b - self.base.b;
        let split = dy * 2 / 3;
        let preview_height = if !document && has_preview(h) {
            self.children.iter().find(|(c,_)|unsafe{GetDlgCtrlID(*c)}==444).map_or(0,|(_,r)|r.b-r.y)
        } else {0};
        let log_space = if document || self.log_expanded {0} else {
            self.children.iter().find(|(c,_)|unsafe{GetDlgCtrlID(*c)}==433).map_or(0,|(_,r)|r.b-r.y)
        };
        for &(child, rect) in &self.children {
            let id = unsafe { GetDlgCtrlID(child) };
            let (x, y, width, height) = if document {
                let sidebar=dx/4;
                match id {120=>(sidebar,0,dx-sidebar,dy), 672=>(0,0,sidebar,dy), 2=>(dx,dy,0,0), _=>continue}
            } else {
                match id {
                    414 | 440 => (0,-log_space,dx,split+log_space),
                    420 | 441 => (0,-log_space,dx,0),
                    435 | 442 | 443 => (dx,-log_space,0,0),
                    438 | 439 | 446 | 449 => (dx,split,0,0),
                    421 => (0,split,0,0),
                    681 => (0,split,dx/3,dy-split),
                    685 => (dx/3,split,dx-dx/3,0),
                    683 => (dx/3,split,dx-dx/3,(dy-split)/2),
                    686 => (dx/3,split+(dy-split)/2,dx-dx/3,0),
                    684 => (dx/3,split+(dy-split)/2,dx-dx/3,dy-split-(dy-split)/2),
                    403 => (0,split,dx,dy-split-preview_height),
                    444 => (0,dy,dx,0),
                    416 | 448 => (dx,0,0,0),
                    447 => (0,0,dx,0),
                    433 | 415 | 111 | 408 | 109 => (0,0,dx,0),
                    409 => (0,0,dx/2,dy),
                    405 => (dx/2,0,dx-dx/2,dy),
                    432 => (dx/2,0,0,0),
                    434 | 413 => (0,dy,dx,0),
                    114 | 404 | 445 => (dx,dy,0,0),
                    2 | 437 | 411 | 412 | 406 => (0,dy,0,0),
                    _ => continue,
                }
            };
            unsafe { MoveWindow(child, rect.x+x, rect.y+y, rect.r-rect.x+width, rect.b-rect.y+height, 1); }
            if id==444 {
                let hosted=unsafe{GetWindow(child,5)};
                if hosted!=0 {let (w,h)=client_size(child); unsafe{MoveWindow(hosted,0,0,w,h,1);}}
            }
        }
    }
}
pub(super) fn has_preview(dialog:H)->bool {
    unsafe { GetWindow(GetDlgItem(dialog,444),5)!=0 }
}
pub(super) fn document_width(h:H,id:i32)->i32 {
    #[link(name="user32")]
    extern "system" {fn GetDpiForWindow(h:H)->u32;}
    let child=unsafe{GetDlgItem(h,id)};
    let dpi=unsafe{GetDpiForWindow(child)}.max(96) as i32;
    ((client_size(child).0-12)*1440/dpi).max(1200)
}
pub(super) fn client_size(h:H) -> (i32,i32) {
    let mut rect = Rect::default();
    unsafe { GetClientRect(h,&mut rect); }
    (rect.r,rect.b)
}
pub fn minimum(ptr: isize) {
    LAYOUT.with(|v| v.borrow().minimum(ptr));
}
pub fn resize(h: H) {
    LAYOUT.with(|v| {
        let l = v.borrow();
        if l.children.is_empty() {
            return;
        }
        let mut now = Rect::default();
        unsafe {
            GetClientRect(h, &mut now);
        }
        let dx = now.r - l.base.r;
        let dy = now.b - l.base.b;
        let flags = PANES.with(|v| v.get());
        let ux = |v: i32| (v * l.base.r + 406) / 812;
        let uy = |v: i32| (v * l.base.b + 230) / 461;
        let left = ux(if flags & 1 == 0 { 160 } else { 38 });
        let top = uy(if flags & 2 == 0 { 128 } else { 29 });
        let right = now.r - ux(8);
        let bottom = now.b - uy(38);
        let both = flags & 12 == 0;
        let stacked = both && flags & 16 != 0;
        for &(c, r) in &l.children {
            let id = unsafe { GetDlgCtrlID(c) };
            let (mut x, mut y, mut w, mut ht) = (r.x, r.y, r.r - r.x, r.b - r.y);
            match pane(id) {
                group @ (4 | 8) => {
                    let doc = group == 8;
                    let (mut x0, mut y0, mut x1, mut y1) = (left, top, right, bottom);
                    if stacked {
                        let middle = (top + bottom) / 2;
                        if doc {
                            y0 = middle + uy(2)
                        } else {
                            y1 = middle - uy(2)
                        }
                    } else if both {
                        let middle = (left + right) / 2;
                        if doc {
                            x0 = middle + ux(3)
                        } else {
                            x1 = middle - ux(3)
                        }
                    }
                    let (old_left, old_right) = if doc { (484, 804) } else { (160, 478) };
                    let dw = x1 - x0 - ux(old_right - old_left);
                    let dh = y1 - y0 - uy(295);
                    x += x0 - ux(old_left);
                    y += y0 - uy(128);
                    if matches!(id, 106 | 118 | 120 | 210 | 211) {
                        w += dw
                    }
                    if matches!(id, 106 | 120 | 210 | 211) {
                        ht += dh
                    }
                    if matches!(id, 156 | 115 | 159 | 113 | 160) {
                        y += dh
                    }
                    if id == 119 {
                        x += dw
                    }
                }
                1 => {
                    if id == 145 {
                        y += dy
                    } else if id == 104 || id == 191 {
                        ht += dy
                    }
                }
                2 => {
                    let old_left = ux(160);
                    let available = now.r - left - ux(8);
                    let scale = |p: i32| left + (p - old_left) * available / ux(644);
                    x = scale(r.x);
                    w = scale(r.r) - x;
                }
                _ => {
                    if matches!(id, 103 | 193 | 122) {
                        x += dx
                    }
                    if id == 123 {
                        w += dx
                    }
                    if r.y > uy(425) {
                        y += dy
                    }
                }
            }
            unsafe {
                MoveWindow(c, x, y, w, ht, 1);
            }
        }
    });
    resize_columns(h);
}
#[link(name = "user32")]
extern "system" {
    fn GetDlgCtrlID(c: H) -> i32;
}
#[repr(C)]
pub struct Header {
    pub hwnd: H,
    pub id: usize,
    pub code: i32,
}
#[link(name = "user32")]
extern "system" {
    fn SetWindowLongPtrW(h: H, index: i32, v: H) -> H;
}
pub fn reject_notify(h: H) {
    unsafe {
        SetWindowLongPtrW(h, 0, 1);
    }
}
#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(i: i32) -> H;
    fn SetBkMode(dc: H, m: i32) -> i32;
}
pub fn background(dc: usize) -> H {
    unsafe {
        SetBkMode(dc as H, 1);
        GetStockObject(0)
    }
}
#[cfg(test)]
pub fn check_resize(h: H) -> Result<()> {
    unsafe {
        capture(h);
        let editor = GetDlgItem(h, 120);
        let mut before = Rect::default();
        GetClientRect(editor, &mut before);
        let mut frame = Rect::default();
        GetWindowRect(h, &mut frame);
        MoveWindow(
            h,
            frame.x,
            frame.y,
            frame.r - frame.x + 160,
            frame.b - frame.y + 100,
            0,
        );
        resize(h);
        let mut after = Rect::default();
        GetClientRect(editor, &mut after);
        if after.r <= before.r || after.b <= before.b {
            return Err("資料ペインがサイズ変更に追従しません".into());
        }
        let bounds = |id| {
            let mut r = Rect::default();
            GetWindowRect(GetDlgItem(h, id), &mut r);
            r
        };
        let wide_before = bounds(210);
        apply_layout(h, 16);
        if bounds(210).b > bounds(211).y || (bounds(210).x - bounds(211).x).abs() > 3 {
            return Err("上下分割の境界が不正です".into());
        }
        apply_layout(h, 8);
        if bounds(210).r <= wide_before.r {
            return Err("キューの幅が広がりません".into());
        }
        apply_layout(h, 0);
        if bounds(210).r > bounds(211).x {
            return Err("左右分割の境界が不正です".into());
        }
        Ok(())
    }
}
thread_local! {static PANES:std::cell::Cell<u8>=const{std::cell::Cell::new(0)};}
fn pane(id: i32) -> u8 {
    match id {
        104 | 127 | 143 | 145 | 142 | 191 => 1,
        105 | 141 | 150..=153 | 155 | 170..=174 | 213..=215 => 2,
        106 | 113 | 115 | 125..=126 | 128 | 156 | 159 | 160 | 210 => 4,
        118..=120 | 175..=177 | 181 | 211 => 8,
        _ => 0,
    }
}
pub fn apply_layout(h: H, flags: u8) {
    PANES.with(|p| p.set(flags));
    LAYOUT.with(|l| {
        for &(c, _) in &l.borrow().children {
            unsafe {
                let group = pane(GetDlgCtrlID(c));
                if group != 0 {
                    ShowWindow(c, if flags & group == 0 { 5 } else { 0 });
                }
            }
        }
    });
    resize(h);
}
#[link(name = "user32")]
extern "system" {
    fn CreatePopupMenu() -> H;
    fn DestroyMenu(menu: H) -> i32;
    fn AppendMenuW(menu: H, flags: u32, id: usize, title: Ptr) -> i32;
    fn TrackPopupMenu(menu: H, flags: u32, x: i32, y: i32, z: i32, owner: H, rect: usize) -> i32;
}
pub fn layout_menu(h: H, flags: u8) -> Option<i32> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return None;
        }
        for (id, name) in [
            (194, "プロジェクト"),
            (195, "プロジェクト別の進捗・KPI"),
            (196, "作業キュー"),
            (197, "タスク資料"),
            (198, "左右に分割"),
            (199, "上下に分割"),
            (200, "標準に戻す"),
        ] {
            if id == 198 || id == 200 {
                AppendMenuW(menu, 0x800, 0, std::ptr::null());
            }
            let checked = match id {
                194..=197 => flags & (1 << (id - 194)) == 0,
                198 => flags & 16 == 0,
                199 => flags & 16 != 0,
                _ => false,
            };
            AppendMenuW(menu, if checked { 8 } else { 0 }, id, wide(name).as_ptr());
        }
        let mut r = Rect::default();
        GetWindowRect(GetDlgItem(h, 193), &mut r);
        let id = TrackPopupMenu(menu, 0x102, r.x, r.b, 0, h, 0);
        DestroyMenu(menu);
        (id != 0).then_some(id)
    }
}
