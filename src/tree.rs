use super::*;
use std::{
    collections::{HashMap, HashSet},
    os::windows::fs::MetadataExt,
};
const TREE: i32 = 104;
const ROOT: H = -65536;
#[repr(C)]
#[derive(Default)]
struct Item {
    mask: u32,
    handle: H,
    state: u32,
    state_mask: u32,
    text: usize,
    max: i32,
    image: i32,
    selected_image: i32,
    children: i32,
    param: H,
}
#[repr(C)]
struct Insert {
    parent: H,
    after: H,
    item: Item,
}
#[repr(C)]
struct Change {
    header: native::Header,
    action: u32,
    old: Item,
    new: Item,
    point: [i32; 2],
}
#[derive(Clone)]
struct Node {
    project: usize,
    path: PathBuf,
    folder: bool,
}
#[derive(Default)]
struct Tree {
    roots: Vec<H>,
    nodes: HashMap<H, Node>,
    loaded: HashSet<H>,
}
thread_local! {static DATA:RefCell<Tree>=RefCell::default();}
#[repr(C)]
struct FileInfo {
    icon: H,
    index: i32,
    attributes: u32,
    name: [u16; 260],
    kind: [u16; 80],
}
#[link(name = "shell32")]
extern "system" {
    fn SHGetFileInfoW(path: Ptr, attr: u32, info: *mut FileInfo, size: u32, flags: u32) -> usize;
}
fn icon(h: H, folder: bool) -> i32 {
    let mut info: FileInfo = unsafe { std::mem::zeroed() };
    let images = unsafe {
        SHGetFileInfoW(
            wide(if folder { "folder" } else { "file.txt" }).as_ptr(),
            if folder { 16 } else { 128 },
            &mut info,
            std::mem::size_of::<FileInfo>() as _,
            0x4011,
        )
    };
    if images != 0 {
        msg(h, TREE, 0x1109, 0, images as _);
    }
    info.index
}
fn insert(h: H, parent: H, node: Node, icon: i32) -> H {
    let name = node
        .path
        .file_name()
        .unwrap_or(node.path.as_os_str())
        .to_string_lossy();
    let name = wide(&name);
    let req = Insert {
        parent,
        after: -65534,
        item: Item {
            mask: 1 | 2 | 32 | 64,
            text: name.as_ptr() as _,
            image: icon,
            selected_image: icon,
            children: node.folder as _,
            ..Default::default()
        },
    };
    let handle = msg(h, TREE, 0x1132, 0, &req as *const _ as _);
    if handle != 0 {
        DATA.with(|d| {
            d.borrow_mut().nodes.insert(handle, node);
        });
    }
    handle
}
pub fn reset(h: H) {
    msg(h, TREE, 0x1101, 0, ROOT);
    DATA.with(|d| *d.borrow_mut() = Tree::default());
}
fn selected(h: H) -> Option<Node> {
    node(msg(h, TREE, 0x110a, 9, 0))
}
pub fn selected_folder(h: H) -> Result<String> {
    let n = selected(h)
        .filter(|n| n.folder)
        .ok_or("ツリーから登録するフォルダーを選択してください")?;
    Ok(n.path.to_string_lossy().into_owned())
}
fn node(handle: H) -> Option<Node> {
    DATA.with(|d| d.borrow().nodes.get(&handle).cloned())
}
pub fn roots(h: H, a: &App) {
    let count = DATA.with(|d| d.borrow().roots.len());
    if count < a.projects.len() {
        let img = icon(h, true);
        for (i, p) in a.projects.iter().enumerate().skip(count) {
            let handle = insert(
                h,
                ROOT,
                Node {
                    project: i,
                    path: p.into(),
                    folder: true,
                },
                img,
            );
            DATA.with(|d| d.borrow_mut().roots.push(handle));
        }
    }
    if selected(h).is_none_or(|n| n.project != a.project) {
        let handle = DATA.with(|d| d.borrow().roots.get(a.project).copied());
        if let Some(handle) = handle {
            msg(h, TREE, 0x110b, 9, handle);
        }
    }
    set(
        h,
        144,
        a.projects
            .get(a.project)
            .map(|p| p.strip_prefix(r"\\?\").unwrap_or(p))
            .unwrap_or("プロジェクトを追加してください"),
    );
}
fn expand(h: H, handle: H) -> Result<()> {
    if DATA.with(|d| d.borrow().loaded.contains(&handle)) {
        return Ok(());
    }
    let Some(parent) = node(handle) else {
        return Ok(());
    };
    let entries = fs::read_dir(&parent.path).map_err(|e| e.to_string())?;
    let mut children = Vec::new();
    for e in entries.flatten() {
        let Ok(m) = e.metadata() else { continue };
        let dir = m.is_dir() && m.file_attributes() & 0x400 == 0;
        children.push((
            !dir,
            e.file_name().to_string_lossy().to_lowercase(),
            e.path(),
        ))
    }
    children.sort();
    let folder = icon(h, true);
    let file = icon(h, false);
    for (not_dir, _, path) in children {
        insert(
            h,
            handle,
            Node {
                project: parent.project,
                path,
                folder: !not_dir,
            },
            if not_dir { file } else { folder },
        );
    }
    let first = msg(h, TREE, 0x110a, 4, handle);
    if first == 0 {
        let item = Item {
            mask: 64,
            handle,
            children: 0,
            ..Default::default()
        };
        msg(h, TREE, 0x113f, 0, &item as *const _ as _);
    }
    DATA.with(|d| d.borrow_mut().loaded.insert(handle));
    Ok(())
}
pub fn notification(h: H, code: i32, l: isize) -> isize {
    if !matches!(code, -454 | -450 | -451) {
        return 0;
    }
    let change = unsafe { &*(l as *const Change) };
    let Some(n) = node(change.new.handle) else {
        return 0;
    };
    if code == -454 {
        if change.action & 2 != 0 {
            if let Err(e) = expand(h, change.new.handle) {
                notice(h, &e);
                native::reject_notify(h);
                return 1;
            }
        }
        return 0;
    }
    if code == -451 {
        APP.with(|c| {
            if let Ok(mut a) = c.try_borrow_mut() {
                if a.project != n.project {
                    a.project = n.project;
                    a.task = a.indices().first().copied();
                    refresh(h, &a);
                    render(h, &a);
                }
                if !n.folder
                    && n.path.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown")
                    })
                {
                    if let Ok(rel) = n.path.strip_prefix(&a.projects[n.project]) {
                        set(h, 118, &rel.to_string_lossy());
                    }
                    attach_state(h, &a);
                    display_document(h, Ok(n.path.clone()));
                    set(h, 141, "ツリーの文書を表示中");
                }
                set(h, 144, n.path.to_string_lossy().trim_start_matches(r"\\?\"));
            }
        });
    }
    0
}
#[cfg(test)]
pub fn check(h: H) -> Result<()> {
    let root = msg(h, TREE, 0x110a, 0, 0);
    if root == 0 {
        return Err("ルート未登録".into());
    }
    msg(h, TREE, 0x1102, 2, root);
    let count = msg(h, TREE, 0x1105, 0, 0);
    expand(h, root)?;
    if count < 2 || count != msg(h, TREE, 0x1105, 0, 0) {
        return Err("子項目の展開に失敗".into());
    }
    let document = DATA
        .with(|d| {
            d.borrow()
                .nodes
                .iter()
                .find(|(_, n)| n.path.ends_with("result.md"))
                .map(|(k, _)| *k)
        })
        .ok_or("文書ノードなし")?;
    msg(h, TREE, 0x110b, 9, document);
    if !get(h, 120).contains("確認済み") {
        return Err("ツリー文書プレビュー失敗".into());
    }
    msg(h, TREE, 0x110b, 9, root);
    Ok(())
}
