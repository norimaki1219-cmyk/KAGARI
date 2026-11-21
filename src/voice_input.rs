use super::*;

// INPUT's union is sized and aligned for the largest of the native input records.
#[repr(C)]
#[derive(Clone, Copy)]
struct Keyboard {
    key: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Mouse {
    x: i32,
    y: i32,
    data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
union Payload {
    keyboard: Keyboard,
    mouse: Mouse,
}
#[repr(C)]
struct Input {
    kind: u32,
    payload: Payload,
}
#[link(name = "user32")]
extern "system" {
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    fn GetForegroundWindow() -> H;
    fn GetFocus() -> H;
}
fn key(vk: u16, release: bool) -> Input {
    Input {
        kind: 1,
        payload: Payload {
            keyboard: Keyboard {
                key: vk,
                scan: 0,
                flags: if release { 2 } else { 0 },
                time: 0,
                extra: 0,
            },
        },
    }
}
pub(super) fn start(dialog: H) -> Result<()> {
    const FIELD: i32 = 403;
    let edit = unsafe { GetDlgItem(dialog, FIELD) };
    let mut caret = 0u32;
    msg(dialog, FIELD, 0xb0, 0, &mut caret as *mut _ as isize); // EM_GETSEL
    unsafe {
        SetFocus(edit);
    }
    msg(dialog, FIELD, 0xb1, caret as usize, caret as isize); // preserve existing draft
    if unsafe { GetFocus() } != edit {
        return Err("依頼欄にカーソルを置き、Windows キー＋H を押してください。".into());
    }
    #[cfg(test)]
    if TEST_SHORTCUT.with(|flag| flag.replace(false)) {
        return Ok(());
    }
    if unsafe { GetForegroundWindow() } != dialog {
        return Err("タスク詳細を前面にしてから音声入力を開始してください。".into());
    }
    let shortcut = [
        key(0x5b, false),
        key(0x48, false),
        key(0x48, true),
        key(0x5b, true),
    ];
    let size = std::mem::size_of::<Input>() as i32;
    let sent = unsafe { SendInput(4, shortcut.as_ptr(), size) };
    if sent != 4 {
        if sent != 0 {
            let releases = &shortcut[if sent == 2 { 2 } else { 3 }..];
            unsafe {
                SendInput(releases.len() as u32, releases.as_ptr(), size);
            }
        }
        return Err(
            "Windows 音声入力を起動できませんでした。依頼欄で Windows キー＋H を押してください。"
                .into(),
        );
    }
    Ok(())
}
#[cfg(test)]
thread_local! {pub(super) static TEST_SHORTCUT:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
