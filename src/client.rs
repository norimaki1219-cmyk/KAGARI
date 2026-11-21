use super::*;
fn release(path: &Path, prefix: &str) -> Vec<u64> {
    path.file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix(prefix))
        .unwrap_or("")
        .split('-')
        .next()
        .unwrap_or("")
        .split('.')
        .filter_map(|n| n.parse().ok())
        .collect()
}
pub fn find(agent: usize) -> Result<PathBuf> {
    let name = if agent == 0 { "codex" } else { "claude" };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|s| {
            std::env::split_paths(&s)
                .filter(|p| p.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    if let Some(p) = &home {
        dirs.push(p.join(".local/bin"))
    }
    if let Some(p) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(p).join("npm"))
    }
    let mut preferred = None;
    'search: for dir in dirs {
        for ext in ["exe", "cmd"] {
            let p = dir.join(format!("{name}.{ext}"));
            if p.is_file() {
                if agent == 0 { return Ok(p); }
                preferred = Some(p);
                break 'search;
            }
        }
    }
    let mut bundled = Vec::new();
    if let Some(home) = home {
        let prefix = if agent == 0 {
            "openai.chatgpt-"
        } else {
            "anthropic.claude-code-"
        };
        for root in [".vscode/extensions", ".vscode-insiders/extensions"] {
            let Ok(entries) = fs::read_dir(home.join(root)) else {
                continue;
            };
            let mut versions: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with(prefix)
                })
                .collect();
            versions.sort_by_key(|p| std::cmp::Reverse(release(p, prefix)));
            for p in versions {
                let bin = if agent == 0 {
                    p.join(if cfg!(target_arch = "aarch64") {
                        "bin/windows-aarch64/codex.exe"
                    } else {
                        "bin/windows-x86_64/codex.exe"
                    })
                } else {
                    p.join("resources/native-binary/claude.exe")
                };
                if bin.is_file() {
                    if agent == 0 { return Ok(bin); }
                    bundled.push((release(&p, prefix), bin));
                }
            }
        }
    }
    bundled.sort_by(|a,b| b.0.cmp(&a.0));
    if let Some((version, bin)) = bundled.into_iter().next() {
        if let Some(path) = preferred.as_ref() {
            let current = Command::new(path).arg("--version").creation_flags(0x08000000)
                .output().ok().filter(|o| o.status.success())
                .and_then(|o| {
                    let text = String::from_utf8(o.stdout).ok()?;
                    text.split_whitespace().next()?.split('.')
                        .map(str::parse::<u64>).collect::<std::result::Result<Vec<_>, _>>().ok()
                });
            if current.is_some_and(|current| current >= version) { return Ok(path.clone()); }
        }
        return Ok(bin);
    }
    if let Some(path) = preferred { return Ok(path); }
    Err(format!("{name} が見つかりません。CLIまたは対応するVS Code拡張をインストールしてから、もう一度実行してください。"))
}
