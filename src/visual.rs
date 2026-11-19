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
pub fn legend(dc: usize, hwnd: H) -> H {
    let brush = native::background(dc);
    let id = unsafe { GetDlgCtrlID(hwnd) };
    if (170..174).contains(&id) {
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
