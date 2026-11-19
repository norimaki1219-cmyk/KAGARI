use super::*;
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    pub b: i32,
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
#[link(name = "user32")]
extern "system" {
    fn LoadImageW(instance: H, name: Ptr, kind: u32, w: i32, h: i32, flags: u32) -> H;
    fn DrawIconEx(
        dc: H,
        x: i32,
        y: i32,
        icon: H,
        w: i32,
        h: i32,
        step: u32,
        brush: H,
        flags: u32,
    ) -> i32;
    fn GetSysColorBrush(i: i32) -> H;
    fn GetSysColor(i: i32) -> u32;
    fn FillRect(dc: H, r: &Rect, brush: H) -> i32;
    fn DrawTextW(dc: H, s: Ptr, len: i32, r: *mut Rect, f: u32) -> i32;
    fn DrawFocusRect(dc: H, r: &Rect) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn SetBkMode(dc: H, mode: i32) -> i32;
    fn SetTextColor(dc: H, color: u32) -> u32;
}
fn paint(dc: H, r: Rect, brand: Option<usize>, selected: bool, label: Option<&str>) {
    unsafe {
        let bg = if selected { 13 } else { 5 };
        FillRect(dc, &r, GetSysColorBrush(bg));
        let size = (r.b - r.y - 2).clamp(10, 32);
        let mut tx = r.x + 5;
        if let Some(index) = brand {
            let ic = LoadImageW(
                GetModuleHandleW(std::ptr::null()),
                (10 + index) as Ptr,
                1,
                size,
                size,
                0x8000,
            );
            DrawIconEx(
                dc,
                if label.is_some() {
                    tx
                } else {
                    (r.x + r.r - size) / 2
                },
                (r.y + r.b - size) / 2,
                ic,
                size,
                size,
                0,
                0,
                3,
            );
            tx += size + 5;
        }
        if let Some(s) = label {
            SetBkMode(dc, 1);
            SetTextColor(dc, GetSysColor(if selected { 14 } else { 8 }));
            let mut area = Rect { x: tx, ..r };
            DrawTextW(dc, wide(s).as_ptr(), -1, &mut area, 0x8824);
        }
    }
}
pub fn draw(l: isize) {
    let d = unsafe { &*(l as *const Draw) };
    if d.index == u32::MAX {
        return;
    }
    let index = if d.id == 125 {
        (d.index as usize).checked_sub(1)
    } else {
        Some(d.index as usize)
    };
    let label = index
        .and_then(|i| AGENTS.get(i).copied())
        .unwrap_or("全担当");
    let closed = d.state & 0x1000 != 0;
    paint(
        d.dc,
        d.rect,
        index,
        d.state & 1 != 0,
        if closed && index.is_some() {
            None
        } else {
            Some(label)
        },
    );
    if d.state & 16 != 0 {
        unsafe {
            DrawFocusRect(d.dc, &d.rect);
        }
    }
}
pub fn list(h: H, dc: H, row: usize) {
    let mut r = Rect {
        x: 0,
        y: 3,
        ..Default::default()
    };
    if msg(h, 106, 0x1038, row, &mut r as *mut _ as _) == 0 {
        return;
    }
    let mut text = [0u16; 40];
    let mut item = super::native::brand_item(text.as_mut_ptr(), text.len());
    msg(h, 106, 0x1073, row, &mut item as *mut _ as _);
    let name =
        String::from_utf16_lossy(&text[..text.iter().position(|&x| x == 0).unwrap_or(text.len())]);
    let index = AGENTS.iter().position(|&n| n == name);
    paint(dc, r, index, msg(h, 106, 0x102c, row, 2) != 0, None);
}
