use super::*;
#[repr(C)]
struct Browse {
    owner: H,
    root: H,
    name: *mut u16,
    title: Ptr,
    flags: u32,
    callback: H,
    param: H,
    image: i32,
}
#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(b: *mut Browse) -> H;
    fn SHGetPathFromIDListW(p: H, b: *mut u16) -> i32;
}
#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(p: H);
    fn OleInitialize(p: H) -> i32;
    fn OleUninitialize();
}
fn value(b: &[u16]) -> String {
    String::from_utf16_lossy(&b[..b.iter().position(|x| *x == 0).unwrap_or(b.len())])
}
pub fn folder(h: H) -> Option<String> {
    unsafe {
        let initialized = OleInitialize(0) >= 0;
        let mut buf = [0u16; 260];
        let title = wide("プロジェクトフォルダーを選択");
        let mut b = Browse {
            owner: h,
            root: 0,
            name: buf.as_mut_ptr(),
            title: title.as_ptr(),
            flags: 0x51,
            callback: 0,
            param: 0,
            image: 0,
        };
        let p = SHBrowseForFolderW(&mut b);
        let path = if p != 0 {
            let ok = SHGetPathFromIDListW(p, buf.as_mut_ptr());
            CoTaskMemFree(p);
            (ok != 0).then(|| value(&buf))
        } else {
            None
        };
        if initialized {
            OleUninitialize()
        }
        path
    }
}
