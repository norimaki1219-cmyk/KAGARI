/// Text from the two most recent distinct answers, newest first.
pub struct Reply {
    pub time: u128,
    pub text: String,
    pub previous: String,
}

fn has(text: &str, words: &[&str]) -> bool {
    words.iter().any(|word| text.contains(word))
}

fn clean(line: &str) -> String {
    line.trim().trim_start_matches('#').trim().replace("**", "")
}

fn extract(text: &str) -> Vec<String> {
    let mut actions = Vec::new();
    let mut section = false;
    let mut in_code = false;
    for raw in text.lines() {
        let line = clean(raw);
        if line.starts_with("```") || line.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code || line.starts_with('>') || line.starts_with("- [x]") {
            continue;
        }
        let is_heading = raw.trim_start().starts_with('#')
            || line.ends_with(['：', ':'])
            || (raw.trim().starts_with("**") && raw.trim().ends_with("**"));
        let heading = line.trim_end_matches(['：', ':']).trim();
        if has(
            heading,
            &[
                "次のアクション",
                "次にすること",
                "次のステップ",
                "残タスク",
                "残作業",
                "Next steps",
                "Next actions",
            ],
        ) && heading.chars().count() < 22
            && !heading.contains(['。', '：', ':'])
        {
            section = true;
            continue;
        }
        if is_heading || line.is_empty() {
            if is_heading {
                section = false;
            }
            if line.is_empty() {
                continue;
            }
        }
        if line.starts_with('[') && line.contains("](") {
            continue;
        }
        let item = line
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim();
        if item.is_empty() {
            continue;
        }
        if section || item.starts_with("[ ]") || has(item, &["優先1", "優先2", "優先3"]) {
            actions.push(item.trim_start_matches("[ ]").trim().to_string());
        } else {
            for sentence in item.split_inclusive(['。', '！', '？']) {
                let sentence = sentence.trim();
                if has(
                    sentence,
                    &[
                        "指示を待",
                        "承認を待",
                        "返答を待",
                        "承認待ち",
                        "判断待ち",
                        "してください",
                        "して下さい",
                        "する必要",
                        "する予定",
                        "今すぐ着手",
                        "未確認です",
                        "対応は不要",
                        "対応不要",
                        "次に",
                        "次は",
                        "残っています",
                        "が必要です",
                    ],
                ) {
                    actions.push(sentence.to_string());
                }
            }
        }
    }
    actions
}

pub fn conversation(reply: &Reply) -> String {
    let mut prose = Vec::new();
    let mut fenced = false;
    for raw in reply.text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") { fenced = !fenced; continue; }
        if fenced || line.is_empty() || line.starts_with(['#','|','>']) { continue; }
        let line = clean(line).trim_start_matches("- ").to_owned();
        if !line.is_empty() { prose.push(line); }
        if prose.len() == 2 { break; }
    }
    let excerpt = if prose.is_empty() { clean(&reply.text) } else { prose.join("\n") };
    let mut chars = excerpt.chars();
    let mut text: String = chars.by_ref().take(320).collect();
    if chars.next().is_some() { text.push('…'); }
    let actions = summary(reply);
    for line in actions.lines().filter(|s|s.starts_with('・')) {
        if !text.contains(line.trim_start_matches('・')) { text.push('\n'); text.push_str(line); }
    }
    // A selected introduction and its Markdown list form one unit. Keep the
    // following items (including nested items) together in the compact view.
    let source: Vec<_> = reply.text.lines().collect();
    let mut expanded = Vec::new();
    for selected in text.lines() {
        expanded.push(selected.to_owned());
        let phrase = selected.trim_start_matches('・').trim();
        if phrase.is_empty() { continue; }
        let Some(index) = source.iter().position(|raw| clean(raw).trim_end().ends_with(phrase)) else { continue; };
        let parent = source[index];
        let parent_indent = parent.len()-parent.trim_start().len();
        let mut items = Vec::new();
        for line in source.iter().skip(index+1) {
            let trimmed=line.trim_start();
            if trimmed.is_empty() { continue; }
            let numbered=trimmed.trim_start_matches(|c:char| c.is_ascii_digit());
            let is_item=trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") ||
                (numbered.len()<trimmed.len() && (numbered.starts_with(". ") || numbered.starts_with(") ")));
            if !is_item {break;}
            let nested=line.len()-trimmed.len()>parent_indent;
            if !nested && !(phrase.ends_with(['：',':']) || has(phrase,&["次の","以下の"])) {break;}
            items.push(line.trim_end().to_owned());
        }
        expanded.extend(items);
    }
    expanded.join("\n")
}

pub fn summary(reply: &Reply) -> String {
    let mut actions = extract(&reply.text);
    let prior = actions.is_empty()
        && reply.text.chars().count() < 160
        && has(
            &reply.text,
            &["了解", "承知", "わかりました", "分かりました", "別スレッド"],
        )
        && !has(
            &reply.text,
            &[
                "中止",
                "取り消",
                "キャンセル",
                "完了",
                "不要",
                "保留",
                "停止",
            ],
        );
    if prior {
        actions = extract(&reply.previous);
    }
    if actions.is_empty() {
        return if has(&reply.text, &["完了しました", "対応済み", "終了しました"]) {
            "完了報告です。回答に次の作業の指定はありません。"
        } else {
            "回答に明確な次の作業はありません。必要なら「次のアクション」に記入してください。"
        }
        .into();
    }
    // Surface priorities and blockers before the detailed implementation instructions.
    actions.sort_by_key(|s| {
        if has(s, &["優先1", "優先2", "優先3"]) {
            0
        } else if has(
            s,
            &["未確認", "承認待ち", "指示を待", "対応は不要", "対応不要"],
        ) {
            1
        } else {
            2
        }
    });
    let mut lines = Vec::new();
    for action in actions {
        let mut chars = action.chars();
        let mut short: String = chars.by_ref().take(160).collect();
        if chars.next().is_some() {
            short.push('…');
        }
        let line = format!("・{short}");
        if !lines.contains(&line) {
            lines.push(line);
        }
        if lines.len() == 3 {
            break;
        }
    }
    let prefix = if prior {
        "直前の具体的な回答より（現在の状態は要確認）\n"
    } else {
        ""
    };
    format!("{prefix}{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(text: &str, previous: &str) -> String {
        summary(&Reply {
            time: 0,
            text: text.into(),
            previous: previous.into(),
        })
    }
    #[test]
    fn summary_retains_all_items_after_a_selected_introduction() {
        let reply=Reply{time:0,previous:String::new(),text:"ファイルを確認しました。\n内容は一致しています。\n- そのため、次の3つが残っています。\n  1. 記録\n  2. 更新\n  3. 共有\n\n## 別の話題".into()};
        let view=conversation(&reply);
        assert!(view.contains("1. 記録\n  2. 更新\n  3. 共有"));
    }
    #[test]
    fn wait_is_an_action_and_does_not_revive_old_work() {
        let s = run(
            "了解しました。オーナーの具体的指示を待つ方針です。追加対応は不要です。",
            "古い資料を送ってください。",
        );
        assert!(s.contains("指示を待") && !s.contains("古い"));
    }
    #[test]
    fn acknowledgment_surfaces_previous_priorities() {
        let s = run(
            "了解しました。このスレッドは開発用に残します。",
            "1. **優先1｜店舗を制作**\n2. **優先2｜塔の軽量化**\n原本を渡す許可は未確認です。",
        );
        assert!(s.contains("店舗") && s.contains("軽量化") && s.contains("許可"));
        assert!(!run("完了しました。", "古い資料を送ってください。").contains("古い"));
    }
}
