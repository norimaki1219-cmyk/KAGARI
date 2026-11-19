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
        (124, "すべて"),
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
struct Layout {
    base: Rect,
    outer: Rect,
    children: Vec<(H, Rect)>,
}
thread_local! {static LAYOUT:RefCell<Layout>=RefCell::default();}
pub fn capture(h: H) {
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
        LAYOUT.with(|v| *v.borrow_mut() = l);
    }
}
pub fn minimum(ptr: isize) {
    LAYOUT.with(|v| {
        let r = v.borrow().outer;
        if r.r > r.x {
            unsafe {
                let p = ptr as *mut i32;
                *p.add(6) = r.r - r.x;
                *p.add(7) = r.b - r.y;
            }
        }
    })
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
        105 | 141 | 150..=153 | 155 | 170..=173 | 213..=215 => 2,
        106 | 113 | 115 | 124..=126 | 128 | 156 | 159 | 160 | 210 | 212 => 4,
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
            (195, "部署別の進捗・KPI"),
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
