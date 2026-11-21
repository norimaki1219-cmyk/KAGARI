#[link(name="user32")]
extern "system" {
    fn SetWindowLongPtrW(h:H,index:i32,value:isize)->isize;
    fn GetWindowLongPtrW(h:H,index:i32)->isize;
}
use super::*;

fn markdown_target(text: &str) -> Option<&str> {
    let path = text.split('#').next()?.trim().trim_matches(['<', '>']);
    let p = Path::new(path);
    if path.contains("://") || path.starts_with("//") || path.starts_with(r"\\") {
        return None;
    }
    if let Some((drive, rest)) = path.split_once(':') {
        if drive.len() != 1 || !drive.as_bytes()[0].is_ascii_alphabetic() || rest.contains(':') {
            return None;
        }
    }
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
        .then_some(path)
}

// Return the displayed span and file destination. Keep the original answer intact.
pub(super) fn token(s: &str) -> Option<(usize, &str)> {
    if let Some(rest) = s.strip_prefix('`') {
        let end = rest.find('`')?;
        return Some((end + 2, markdown_target(&rest[..end])?));
    }
    if s.starts_with('[') {
        let start = s.find("](")? + 2;
        let mut depth = 1;
        for (i, ch) in s[start..].char_indices() {
            if ch == '(' {
                depth += 1;
            }
            if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Some((start + i + 1, markdown_target(&s[start..start + i])?));
                }
            }
        }
        return None;
    }
    let end = s
        .find(|c: char| c.is_whitespace() || "`[]()<>「」、。：".contains(c))
        .unwrap_or(s.len());
    let word = s[..end].trim_end_matches([',', ';', '.']);
    Some((word.len(), markdown_target(word)?))
}

pub(super) fn links(text: &str, mut found: impl FnMut(i32, i32, &str)) {
    let mut offset = 0;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for line in normalized.lines() {
        let mut rest = line;
        let mut boundary = true;
        while !rest.is_empty() {
            let ch = rest.chars().next().unwrap();
            if boundary || matches!(ch, '[' | '`') {
                if let Some((len, target)) = token(rest) {
                    let end = offset + rest[..len].encode_utf16().count() as i32;
                    found(offset, end, target);
                    rest = &rest[len..];
                    offset = end;
                    boundary = true;
                    continue;
                }
            }
            boundary = ch.is_whitespace() || "「(：:".contains(ch);
            rest = &rest[ch.len_utf8()..];
            offset += ch.len_utf16() as i32;
        }
        offset += 1; // Rich Edit counts a paragraph break as one character.
    }
}

#[repr(C)]
struct CharFormat {
    size: u32,
    mask: u32,
    effects: u32,
    height: i32,
    offset: i32,
    color: u32,
    charset: u8,
    pitch: u8,
    face: [u16; 32],
}
pub(super) fn text_height(h:H,id:i32,start:i32)->i32 {
    let mut format:CharFormat=unsafe{std::mem::zeroed()};
    format.size=std::mem::size_of::<CharFormat>() as u32;
    msg(h,id,0xb1,start as usize,(start+1) as isize);
    msg(h,id,0x43a,1,&mut format as *mut _ as isize);
    format.height
}
pub fn style_ranges(h:H,id:i32,ranges:&[(i32,i32,String)]) {
    let mut format = CharFormat {
        size: std::mem::size_of::<CharFormat>() as u32,
        mask: 32,
        effects: 0,
        height: 0,
        offset: 0,
        color: 0,
        charset: 0,
        pitch: 0,
        face: [0; 32],
    };
    msg(h, id, 0x445, 0, 0x04000000 | msg(h, id, 0x43b, 0, 0)); // ENM_LINK
    msg(h, id, 0xb1, 0, -1);
    msg(h, id, 0x444, 1, &format as *const _ as isize);
    format.effects = 32; // CFE_LINK
    for &(start,end,_) in ranges {
        msg(h, id, 0xb1, start as usize, end as isize);
        msg(h, id, 0x444, 1, &format as *const _ as isize);
    }
    msg(h, id, 0xb1, 0, 0);
}
fn resolve(task: &Task, target: &str) -> Result<PathBuf> {
    let project = Path::new(&task.project)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let scope = project
        .ancestors()
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case(".company"))
        })
        .unwrap_or(&project);
    let target = Path::new(target);
    let mut candidates = vec![project.join(target)];
    if !target.is_absolute() {
        candidates.push(Path::new(task.working_dir()).join(target));
        candidates.push(scope.join(target));
    }
    for path in candidates {
        if let Ok(p) = path.canonicalize() {
            if p.starts_with(scope) && p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(format!(
        "Markdownファイルを開けません。場所を確認してください。\n{}",
        target.display()
    ))
}
#[repr(C, packed(4))]
struct LinkEvent {
    hwnd: H,
    id: usize,
    code: u32,
    #[cfg(target_pointer_width = "64")]
    header_padding: u32,
    message: u32,
    w: usize,
    l: isize,
    start: i32,
    end: i32,
}
pub fn click(h: H, l: isize, task: &Task, rendered:&[(i32,i32,String)]) -> isize {
    let e = unsafe { &*(l as *const LinkEvent) };
    if e.code != 0x70b || !matches!(e.id, 414 | 409) || e.message != 0x202 {
        return 0;
    }
    let position=e.start;
    let path=rendered.iter().find(|(start,end,_)| *start<=position && position<*end)
        .map(|(_,_,target)| resolve(task,target));
    match path {
        Some(Ok(path)) => unsafe {
            let mut state = (path, native::Layout::default());
            DialogBoxParamW(
                GetModuleHandleW(std::ptr::null()),
                650 as Ptr,
                h,
                preview,
                &mut state as *mut _ as isize,
            );
        },
        Some(Err(err)) => notice(h, &err),
        None => return 0,
    }
    1
}
#[cfg(test)]
thread_local! {pub(super) static TEST_PREVIEW:std::cell::RefCell<Option<fn(H)>>=const{std::cell::RefCell::new(None)};}
extern "system" fn preview(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x110 {
        let (path, layout) = unsafe { &mut *(l as *mut (PathBuf,native::Layout)) };
        *layout = native::Layout::capture(h);
        unsafe { SetWindowLongPtrW(h,-21,l); }
        unsafe {
            SetWindowTextW(
                h,
                wide(&format!(
                    "{} — KAGARI",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ))
                .as_ptr(),
            );
        }
        display_document(h, Ok(path.clone()));
        #[cfg(test)]
        if let Some(action) = TEST_PREVIEW.with(|cb| cb.borrow_mut().take()) {
            action(h);
        }
        return 1;
    }
    if m==0x111 && w&65535==672 && w>>16==1 {
        let index=msg(h,672,0x188,0,0); // LB_GETCURSEL
        if index<0 {return 1;}
        let pos=msg(h,672,0x199,index as usize,0); // LB_GETITEMDATA
        if pos>=0 {
            msg(h,120,0xb1,pos as usize,pos);
            msg(h,120,0xb7,0,0);
            let line=msg(h,120,0x436,0,pos);
            let first=msg(h,120,0xce,0,0);
            msg(h,120,0xb6,0,line-first);
        }
        return 1;
    }
    if matches!(m,5|0x24) {
        let ptr = unsafe {GetWindowLongPtrW(h,-21)} as *const (PathBuf,native::Layout);
        if let Some((_,layout)) = unsafe {ptr.as_ref()} {
            if m==5 {layout.resize_task(h,true);} else {layout.minimum(l);}
        }
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        unsafe {
            EndDialog(h, 0);
        }
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_is_outside_the_clickable_drive_path() {
        let path = r"D:\.company\KasaneFactoryOS\ロードマップ_5年_月別タスク_20261121.md";
        for label in ["全文：", "全文： ", "全文:", "全文: "] {
            let input = format!("{label}{path}");
            let mut matches = Vec::new();
            links(&input, |start, _, target| {
                matches.push((start, target.to_owned()))
            });
            assert_eq!(
                matches,
                vec![(label.encode_utf16().count() as i32, path.to_owned())]
            );
        }
    }
    #[test]
    fn detects_the_same_japanese_document_in_both_reply_formats() {
        let text = r"`D:\.company\KasaneFactoryOS\月別タスク.md`
リンク：[月別タスク.md](KasaneFactoryOS/月別タスク.md)";
        let mut targets = Vec::new();
        links(text, |_, _, path| targets.push(path.to_owned()));
        assert_eq!(
            targets,
            [
                r"D:\.company\KasaneFactoryOS\月別タスク.md",
                "KasaneFactoryOS/月別タスク.md"
            ]
        );
    }
}
