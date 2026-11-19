use super::*;
use std::collections::BTreeMap;
const ID: i32 = 155;
pub const COLORS: [u32; 4] = [0x766959, 0xa35816, 0x12508d, 0x48702a];
#[derive(Clone)]
struct Row {
    name: String,
    counts: [usize; 4],
}
thread_local! {static ROWS:RefCell<Vec<Row>>=RefCell::default();}
fn aggregate(a: &App) -> Vec<Row> {
    let mut groups = BTreeMap::new();
    for t in &a.tasks {
        if Some(&t.project) == a.projects.get(a.project)
            && (a.agent_filter == 0 || a.agent_filter == t.agent + 1)
        {
            let counts = groups.entry(t.dept.clone()).or_insert([0; 4]);
            counts[t.status] += 1;
        }
    }
    groups
        .into_iter()
        .map(|(name, counts)| Row { name, counts })
        .collect()
}
fn label(r: &Row) -> &str {
    if r.name.is_empty() {
        "部署未設定"
    } else {
        &r.name
    }
}
pub fn update(h: H, a: &App) {
    let rows = aggregate(a);
    let top = msg(h, ID, 0x18e, 0, 0);
    msg(h, ID, 0xb, 0, 0);
    msg(h, ID, 0x184, 0, 0);
    let mut unit = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 13,
    };
    unsafe {
        MapDialogRect(h, &mut unit);
    }
    msg(h, ID, 0x1a0, 0, unit.bottom.max(16) as _);
    for (i, r) in rows.iter().enumerate() {
        let desc = format!(
            "{}　未着手 {}　作業中 {}　確認待ち {}　完了 {}",
            label(r),
            r.counts[0],
            r.counts[1],
            r.counts[2],
            r.counts[3]
        );
        msg(h, ID, 0x180, 0, wide(&desc).as_ptr() as _);
        if a.unassigned && r.name.is_empty() || !r.name.is_empty() && r.name == a.department_filter
        {
            msg(h, ID, 0x186, i, 0);
        }
    }
    if rows.is_empty() {
        msg(
            h,
            ID,
            0x180,
            0,
            wide("登録されたタスクはありません").as_ptr() as _,
        );
    }
    ROWS.with(|v| *v.borrow_mut() = rows);
    msg(h, ID, 0x197, top.max(0) as _, 0);
    msg(h, ID, 0xb, 1, 0);
    unsafe {
        InvalidateRect(GetDlgItem(h, ID), std::ptr::null(), 1);
    }
}
pub fn selection(h: H) -> Option<String> {
    let i = msg(h, ID, 0x188, 0, 0);
    ROWS.with(|v| v.borrow().get(i as usize).map(|r| r.name.clone()))
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[link(name = "user32")]
extern "system" {
    fn MapDialogRect(h: H, r: *mut Rect) -> i32;
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
    fn DrawTextW(dc: H, s: Ptr, len: i32, r: *mut Rect, flags: u32) -> i32;
    fn FillRect(dc: H, r: &Rect, b: H) -> i32;
    fn InvalidateRect(h: H, r: *const Rect, erase: i32) -> i32;
    fn DrawFocusRect(dc: H, r: &Rect) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(rgb: u32) -> H;
    fn DeleteObject(obj: H) -> i32;
    fn SetTextColor(dc: H, color: u32) -> u32;
    fn SetBkMode(dc: H, mode: i32) -> i32;
}
fn fill(dc: H, r: Rect, c: u32) {
    unsafe {
        let b = CreateSolidBrush(c);
        FillRect(dc, &r, b);
        DeleteObject(b);
    }
}
fn text(dc: H, mut r: Rect, s: &str, c: u32, align: u32) {
    unsafe {
        SetTextColor(dc, c);
        SetBkMode(dc, 1);
        DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, 0x8824 | align);
    }
}
pub fn draw(l: isize) {
    let d = unsafe { &*(l as *const Draw) };
    let r = d.rect;
    fill(d.dc, r, if d.state & 1 != 0 { 0xf9ede0 } else { 0xffffff });
    ROWS.with(|v| {
        let rows = v.borrow();
        let Some(row) = rows.get(d.index as usize) else {
            if rows.is_empty() {
                text(
                    d.dc,
                    r,
                    "　タスクを登録すると部署別に表示します",
                    0x70665a,
                    0,
                )
            }
            return;
        };
        let w = r.right - r.left;
        let cut = r.left + w / 5;
        let end = r.left + w * 58 / 100;
        let mut start = cut;
        let total: usize = row.counts.iter().sum();
        let max = rows
            .iter()
            .map(|r| r.counts.iter().sum::<usize>())
            .max()
            .unwrap_or(1);
        let mut sum = 0;
        text(
            d.dc,
            Rect {
                left: r.left + 6,
                right: cut - 6,
                ..r
            },
            label(row),
            0x45382e,
            0,
        );
        for (k, n) in row.counts.iter().enumerate() {
            sum += n;
            let next = cut + ((end - cut) as usize * sum / max) as i32;
            if next > start {
                fill(
                    d.dc,
                    Rect {
                        left: start,
                        right: next,
                        top: r.top + 5,
                        bottom: r.bottom - 5,
                    },
                    COLORS[k],
                );
            }
            start = next;
        }
        let cell = (r.right - end) / 5;
        for (k, n) in row.counts.iter().chain(std::iter::once(&total)).enumerate() {
            text(
                d.dc,
                Rect {
                    left: end + cell * k as i32,
                    right: end + cell * (k as i32 + 1),
                    ..r
                },
                &n.to_string(),
                if k < 4 { COLORS[k] } else { 0x45382e },
                1,
            );
        }
    });
    if d.state & 16 != 0 {
        unsafe {
            DrawFocusRect(d.dc, &r);
        }
    }
}
