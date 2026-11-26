use super::*;
use std::{io::Write, process::Stdio, sync::mpsc};

fn call(config: &Path, post: Option<(&str, &str, &str)>) -> Result<String> {
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windowsの場所が不明です")?)
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut command = Command::new(shell);
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            include_str!("slack.ps1"),
        ])
        .creation_flags(0x08000000)
        .env("KAGARI_SLACK_CONFIG", config)
        .env(
            "KAGARI_SLACK_ACTION",
            if post.is_some() { "post" } else { "channels" },
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some((channel, _, thread)) = post {
        command.env("KAGARI_SLACK_CHANNEL", channel).env("KAGARI_SLACK_THREAD",thread);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let Some((_, text, _)) = post {
        child
            .stdin
            .take()
            .ok_or("送信内容を渡せません")?
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() {
        Ok(value)
    } else {
        Err(if value.is_empty() {
            "Slack連携を確認してください".into()
        } else {
            value
        })
    }
}
struct Compose {
    config: PathBuf,
    channels: Vec<(String, String)>,
    draft: String,
    channel:String,
    thread:String,
    sent:String,
    pending: Option<mpsc::Receiver<Result<String>>>,
}
#[link(name = "user32")]
extern "system" {
    fn SetWindowLongPtrW(h: H, id: i32, value: H) -> H;
    fn GetWindowLongPtrW(h: H, id: i32) -> H;
    fn SetTimer(h: H, id: usize, ms: u32, callback: usize) -> usize;
    fn KillTimer(h: H, id: usize) -> i32;
}
impl Compose {
    fn controls(&self, h: H, busy: bool) {
        let selected = msg(h, 651, 0x147, 0, 0);
        unsafe {
            for id in [651, 652, 655, 2] {
                EnableWindow(GetDlgItem(h, id), (!busy) as i32);
            }
            EnableWindow(
                GetDlgItem(h, 653),
                (!busy && selected >= 0 && !get(h, 652).trim().is_empty()) as i32,
            );
        }
    }
}
extern "system" fn dialog(h: H, m: u32, w: usize, l: isize) -> isize {
    if m == 0x110 {
        native::round_fields(h);
        unsafe {
            SetWindowLongPtrW(h, -21, l);
        }
        let s = unsafe { &*(l as *const Compose) };
        for (id, name) in &s.channels {
            msg(
                h,
                651,
                0x143,
                0,
                wide(&format!("#{name}　({id})")).as_ptr() as _,
            );
        }
        msg(h, 652, 0xc5, 40000, 0);
        if let Some(i)=s.channels.iter().position(|(id,_)| id==&s.channel){msg(h,651,0x14e,i,0);}
        set(h,655,&s.thread);
        set(h, 652, &s.draft);
        s.controls(h, false);
        return 1;
    }
    let ptr = unsafe { GetWindowLongPtrW(h, -21) } as *mut Compose;
    if ptr.is_null() {
        return 0;
    }
    if m == 0x10 || m == 0x111 && w & 65535 == 2 {
        if unsafe { (*ptr).pending.is_none() } {
            unsafe {
                EndDialog(h, 0);
            }
        }
        return 1;
    }
    if m == 0x113 && w == 1 {
        let s = unsafe { &mut *ptr };
        let Some(rx) = &s.pending else {
            return 0;
        };
        let result = match rx.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => return 1,
            Err(_) => Err("送信結果を確認できません。Slackを確認してください。".into()),
        };
        unsafe {
            KillTimer(h, 1);
        }
        s.pending = None;
        match result {
            Ok(link) => unsafe {
                s.sent=link;
                EndDialog(h, 1);
            },
            Err(e) => {
                s.controls(h, false);
                set(h, 654, &e);
            }
        }
        return 1;
    }
    if m != 0x111 {
        return 0;
    }
    let id = w & 65535;
    if (id == 651 && w >> 16 == 1) || (id == 652 && w >> 16 == 0x300) {
        let s = unsafe { &*ptr };
        s.controls(h, s.pending.is_some());
        return 1;
    }
    if id == 653 && w >> 16 == 0 {
        let s = unsafe { &mut *ptr };
        let Some((channel, _)) = s.channels.get(msg(h, 651, 0x147, 0, 0) as usize) else {
            return 1;
        };
        let text = get(h, 652);
        if s.pending.is_some() || text.trim().is_empty() {
            return 1;
        }
        let thread=get(h,655);
        let (channel, config) = (channel.clone(), s.config.clone());
        let (tx, rx) = mpsc::channel();
        s.pending = Some(rx);
        s.controls(h, true);
        set(h, 654, "送信中…");
        std::thread::spawn(move || {
            let _ = tx.send(call(&config, Some((&channel, &text, thread.trim()))));
        });
        unsafe {
            SetTimer(h, 1, 150, 0);
        }
        return 1;
    }
    0
}
fn proposal(reply:&str)->Option<(&str,&str,&str)> {
    let (_,rest)=reply.split_once("```kagari-slack\n")?;
    let (block,_)=rest.split_once("\n```")?;
    let (channel,rest)=block.split_once('\n')?;
    let (thread,body)=rest.split_once('\n')?;
    if channel.trim().is_empty() || body.trim().is_empty(){return None;}
    Some((channel.trim(),if thread.trim()=="-"{""}else{thread.trim()},body))
}
pub fn open(owner: H, draft: &str, full:&str) -> Result<()> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = exe
        .ancestors()
        .find(|p| {
            p.file_name()
                .is_some_and(|s| s.eq_ignore_ascii_case(".company"))
        })
        .ok_or("slack_bot の設定が見つかりません")?;
    // 2026-11-26: slack_bot moved under os\ (D:\.company\CLAUDE.md); the old place is kept as a fallback.
    let config = [r"os\slack_bot\config.json", r"slack_bot\config.json"]
        .iter()
        .map(|p| root.join(p))
        .find(|p| p.is_file())
        .unwrap_or_else(|| root.join(r"os\slack_bot\config.json"));
    let rows = call(&config, None)?;
    let channels: Vec<_> = rows
        .lines()
        .filter_map(|s| s.split_once('\t'))
        .map(|(id, name)| (id.into(), name.into()))
        .collect();
    if channels.is_empty() {
        return Err("slack_bot に送信先チャンネルが登録されていません。".into());
    }
    let normalized=full.replace("\r\n","\n");
    let (channel,thread,body)=proposal(&normalized).unwrap_or(("","",draft));
    let mut state = Compose {
        config,
        channels,
        draft:body.replace("\r\n","\n").replace('\n',"\r\n"),
        channel:channel.into(),thread:thread.into(),sent:String::new(),
        pending: None,
    };
    let n = unsafe {
        DialogBoxParamW(
            GetModuleHandleW(std::ptr::null()),
            660 as Ptr,
            owner,
            dialog,
            &mut state as *mut _ as _,
        )
    };
    if n < 0 {
        return Err("Slack投稿画面を開けません".into());
    }
    if n == 1 {
        notice(owner,&format!("Slackに投稿しました。\n{}",state.sent));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::proposal;
    #[test]
    fn extracts_only_reviewed_slack_fields() {
        let reply = "投稿案\n```kagari-slack\nC123\n1234567890.000001\n<@U123> 公開しました。\n続き\n```\n補足";
        assert_eq!(proposal(reply),Some(("C123","1234567890.000001","<@U123> 公開しました。\n続き")));
        assert_eq!(proposal("```kagari-slack\nC123\n-\n新規\n```"),Some(("C123","","新規")));
        assert_eq!(proposal("過去の回答"),None);
        assert_eq!(proposal("```kagari-slack\nC123\n-\n\n```"),None);
    }
}
