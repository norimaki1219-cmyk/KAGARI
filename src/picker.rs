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
#[repr(C)]
#[derive(Default)]
struct OpenFileName {
    size: u32,
    owner: H,
    instance: H,
    filter: Ptr,
    custom_filter: *mut u16,
    custom_capacity: u32,
    filter_index: u32,
    file: *mut u16,
    capacity: u32,
    file_title: *mut u16,
    title_capacity: u32,
    initial_dir: Ptr,
    title: Ptr,
    flags: u32,
    file_offset: u16,
    extension_offset: u16,
    default_extension: Ptr,
    custom_data: H,
    hook: H,
    template: Ptr,
    reserved: H,
    reserved_value: u32,
    extra_flags: u32,
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(info: *mut OpenFileName) -> i32;
    fn CommDlgExtendedError() -> u32;
}
#[cfg(test)]
thread_local! { pub(super) static TEST_IMAGE: RefCell<Option<PathBuf>> = const { RefCell::new(None) }; }
pub fn image(owner: H) -> Result<Option<PathBuf>> {
    #[cfg(test)]
    if let Some(path) = TEST_IMAGE.with(|slot| slot.borrow_mut().take()) { return Ok(Some(path)); }
    let filter = wide("画像 (PNG・JPEG・GIF・WebP)\0*.png;*.jpg;*.jpeg;*.gif;*.webp\0");
    let title = wide("依頼に添付する画像");
    let mut path = vec![0u16; 32768];
    let mut dialog = OpenFileName {
        size: std::mem::size_of::<OpenFileName>() as u32,
        owner,
        filter: filter.as_ptr(),
        title: title.as_ptr(),
        file: path.as_mut_ptr(),
        capacity: path.len() as u32,
        flags: 0x1000 | 0x800 | 0x8,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        Ok(Some(PathBuf::from(value(&path))))
    } else {
        let code = unsafe { CommDlgExtendedError() };
        if code == 0 {
            Ok(None)
        } else {
            Err(format!("画像の選択に失敗しました ({code})"))
        }
    }
}
