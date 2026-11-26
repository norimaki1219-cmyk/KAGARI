use super::*;
use markdown_preview::Preview;

#[derive(Clone, Default, PartialEq)]
pub(super) struct Question {
    pub(super) title: String,
    pub(super) line: usize,
    choices: Vec<String>,
}
pub(super) fn parse(text: &str) -> Vec<Question> {
    let mut out = Vec::<Question>::new();
    let mut lines = text.lines().enumerate().peekable();
    let mut fence = None;
    let mut decision_section = false;
    while let Some((source_line, line)) = lines.next() {
        let trim = line.trim_start();
        if trim.starts_with("```") || trim.starts_with("~~~") {
            let c = trim.as_bytes()[0];
            let size = trim.bytes().take_while(|b| *b == c).count();
            if fence.is_some_and(|(marker,n)| marker == c && size >= n) && trim[size..].trim().is_empty() {
                fence = None;
            } else if fence.is_none() {
                fence = Some((c,size));
            }
            continue;
        }
        if fence.is_some() { continue; }
        if !lines.peek().is_some_and(|(_, next)| Preview::table_header(line, next)) {
            let heading = trim.trim_start_matches('#').trim().trim_matches('*').trim_end_matches(['：', ':']).trim();
            if matches!(heading, "利用者が決めること" | "決めること" | "判断事項" | "要確認事項") {
                decision_section = true;
            } else if trim.starts_with('#') || (trim.starts_with("**") && trim.ends_with("**"))
                || matches!(heading, "完了事項" | "次の依頼") {
                decision_section = false;
            } else if decision_section {
                let numbered = trim.trim_start_matches(|c:char| c.is_ascii_digit());
                let item = ["- ", "* ", "+ "].iter().find_map(|prefix| trim.strip_prefix(prefix))
                    .or_else(|| (numbered.len() < trim.len()).then(|| numbered.strip_prefix(". ").or_else(|| numbered.strip_prefix(") "))).flatten())
                    .or_else(|| trim.ends_with(['？','?']).then_some(trim));
                if let Some(title) = item.map(|s| s.trim_start_matches("[ ] ").trim().replace("**", ""))
                    .filter(|s| !s.is_empty() && !matches!(s.as_str(),"なし"|"ありません"|"なし。") && !out.iter().any(|q| q.title == *s)) {
                    out.push(Question { title, line: source_line, choices: Vec::new() });
                }
            }
            continue;
        }
        decision_section = false;
        let heads = Preview::cells(line);
        let Some(q) = heads.iter().position(|h| {
            matches!(
                h.trim_matches('*'),
                "決めること" | "決定事項" | "確認事項" | "判断事項" | "質問"
            )
        }) else {
            continue;
        };
        let id = heads
            .iter()
            .position(|h| matches!(h.trim_matches('*'), "#" | "ID" | "番号"));
        let opts: Vec<_> = heads
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                [
                    "選択肢",
                    "候補",
                    "推奨",
                    "おすすめ",
                    "お勧め",
                    "お薦め",
                    "提案",
                ]
                .iter()
                .any(|key| h.contains(key))
            })
            .collect();
        lines.next();
        while lines
            .peek()
            .is_some_and(|(_, l)| l.contains('|') && !l.trim_start().starts_with('#'))
        {
            let (line, row) = lines.next().unwrap();
            let cells = Preview::cells(row);
            let Some(topic) = cells.get(q).filter(|v| !v.is_empty()) else {
                continue;
            };
            let key = id
                .and_then(|i| cells.get(i))
                .map(String::as_str)
                .unwrap_or("");
            let title = format!("{} {}", key, topic).trim().replace("**", "");
            let index = out
                .iter()
                .position(|v| v.title == title)
                .unwrap_or_else(|| {
                    out.push(Question {
                        title,
                        line,
                        choices: Vec::new(),
                    });
                    out.len() - 1
                });
            for &(col, header) in &opts {
                if let Some(s) = cells.get(col) {
                    for part in s
                        .replace("<br/>", "<br>")
                        .replace("<br />", "<br>")
                        .split("<br>")
                    {
                        let part = part.trim().replace("**", "");
                        if part.is_empty() || matches!(part.as_str(), "-" | "—") {
                            continue;
                        }
                        let item = if header.starts_with("選択肢") && header.trim() != "選択肢"
                        {
                            format!("{header}　{part}")
                        } else {
                            part
                        };
                        if !out[index].choices.contains(&item) {
                            out[index].choices.push(item);
                        }
                    }
                }
            }
        }
    }
    out
}

pub(super) struct Form {
    questions: Vec<Question>,
    answers: Vec<String>,
    active: usize,
    choice_rows: Vec<usize>,
}
impl Form {
    pub(super) fn answered(questions: &[Question], draft: &str) -> Vec<bool> {
        questions.iter().map(|q| {
            let prefix = Self::prefix(q);
            draft.lines().any(|line| line.strip_prefix(&prefix).is_some_and(|a| !a.trim().is_empty()))
        }).collect()
    }
    pub(super) fn current_answers(&self, active_answer: &str) -> Vec<bool> {
        self.answers.iter().enumerate().map(|(i, answer)| {
            !(if i == self.active { active_answer } else { answer.as_str() }).trim().is_empty()
        }).collect()
    }
    fn prefix(q: &Question) -> String {
        format!("- {}：", q.title)
    }
    fn load(questions: Vec<Question>, draft: &str) -> Self {
        let answers = questions
            .iter()
            .map(|q| {
                draft
                    .lines()
                    .find_map(|line| line.strip_prefix(&Self::prefix(q)))
                    .unwrap_or("")
                    .to_owned()
            })
            .collect();
        Self {
            questions,
            answers,
            active: 0,
            choice_rows: Vec::new(),
        }
    }
    fn merged(&self, draft: &str) -> String {
        let mut lines: Vec<String> = draft.lines().map(str::to_owned).collect();
        for (q, answer) in self.questions.iter().zip(&self.answers) {
            let prefix = Self::prefix(q);
            let answer = answer.trim().lines().collect::<Vec<_>>().join(" ");
            if let Some(index) = lines.iter().position(|line| line.starts_with(&prefix)) {
                if answer.is_empty() {
                    lines.remove(index);
                } else {
                    lines[index] = format!("{prefix}{answer}");
                }
            } else if !answer.is_empty() {
                lines.push(format!("{prefix}{answer}"));
            }
        }
        lines.join("\r\n")
    }
    fn layout_choices(&mut self,h:H,selected:Option<usize>) {
        #[link(name="user32")]
        extern "system" {fn MoveWindow(h:H,x:i32,y:i32,w:i32,height:i32,repaint:i32)->i32;}
        // Use the native multiline edit's word breaks with the list's actual font and width.
        let (width,height)=native::client_size(unsafe{GetDlgItem(h,683)});
        let measure=unsafe{GetDlgItem(h,687)};
        unsafe{MoveWindow(measure,0,0,(width-6).max(1),height.max(1),0);}
        msg(h,687,0x30,msg(h,683,0x31,0,0) as usize,0);
        msg(h,687,0xd3,3,0); // EM_SETMARGINS
        set(h,687,"行\r\n行");
        let y=|index| (msg(h,687,0xd6,index,0)>>16) as i16 as i32;
        let line_height=(y(3)-y(0)).max(1);
        // A native list item is at most 255 pixels high. Split oversized options
        // into scrollable blocks which all select the same original answer.
        let block_lines=((height.min(255)-2)/line_height).max(1) as usize;
        self.choice_rows.clear();
        msg(h, 683, 0x184, 0, 0);
        for (index,choice) in self.questions[self.active].choices.iter().enumerate() {
            set(h,687,choice);
            let text:Vec<u16>=choice.encode_utf16().collect();
            let count=msg(h,687,0xba,0,0) as usize;
            let mut starts:Vec<_>=(0..count).map(|n|msg(h,687,0xbb,n,0) as usize).collect();
            starts.push(text.len());
            for first in (0..count).step_by(block_lines) {
                let last=(first+block_lines).min(count);
                let mut label=Vec::new();
                for line in starts[first..=last].windows(2) {
                    if !label.is_empty(){label.push(10);}
                    label.extend_from_slice(&text[line[0]..line[1]]);
                }
                label.push(0);
                let row=msg(h,683,0x180,0,label.as_ptr() as isize);
                if row>=0 {
                    self.choice_rows.push(index);
                    msg(h,683,0x1a0,row as usize,(line_height*(last-first) as i32+2) as isize);
                }
            }
        }
        if let Some(row)=self.choice_rows.iter().position(|&index|Some(index)==selected){
            msg(h,683,0x186,row,0);
        }
    }
    pub(super) fn rewrap(&mut self,h:H){
        let row=msg(h,683,0x188,0,0) as usize;
        self.layout_choices(h,self.choice_rows.get(row).copied());
    }
    fn show(&mut self, h: H) {
        let selected=self.questions[self.active].choices.iter().position(|c|c==&self.answers[self.active]);
        self.layout_choices(h,selected);
        set(h, 684, &self.answers[self.active]);
    }
}
impl Form {
    pub(super) fn open(h: H, questions: Vec<Question>) -> Self {
        let mut form = Self::load(questions, &get(h, 403));
        msg(h, 681, 0x184, 0, 0);
        for q in &form.questions {
            msg(h, 681, 0x180, 0, wide(&q.title).as_ptr() as _);
        }
        visual::fit_list_width(h,681,form.questions.iter().map(|q|q.title.as_str()));
        msg(h, 681, 0x186, 0, 0);
        form.show(h);
        form
    }
    pub(super) fn select(&mut self, h: H, id: i32) {
        let index = msg(h, id, 0x188, 0, 0) as usize;
        if id == 681 && index < self.questions.len() {
            self.answers[self.active] = get(h, 684);
            self.active = index;
            self.show(h);
        } else if id == 683 {
            if let Some(&choice) = self.choice_rows.get(index) {
                set(h, 684, &self.questions[self.active].choices[choice]);
            }
        }
    }
    pub(super) fn apply(&mut self, h: H) {
        self.answers[self.active] = get(h, 684);
        let draft = get(h, 403);
        if self.answers != Self::load(self.questions.clone(), &draft).answers {
            let merged=self.merged(&draft);
            if merged!=draft {set(h,403,&merged);}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_and_partial_reply() {
        let q=parse("| # | 決めること | お勧め |\n|---|---|---|\n| W1 | Slackへの送り方 | **a：Webhook**<br>b：手動 |\n| W2 | 宛先 | |");
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].choices, ["a：Webhook", "b：手動"]);
        let mut f = Form::load(q, "先に確認");
        f.answers[1] = "担当者のみ".into();
        let d = f.merged("先に確認");
        assert_eq!(d, "先に確認\r\n- W2 宛先：担当者のみ");
        assert_eq!(Form::load(f.questions, &d).answers, ["", "担当者のみ"]);
    }
}
