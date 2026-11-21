use super::*;
use markdown_preview::Preview;

#[derive(Clone, Default, PartialEq)]
pub(super) struct Question {
    title: String,
    choices: Vec<String>,
}
pub(super) fn parse(text: &str) -> Vec<Question> {
    let mut out = Vec::<Question>::new();
    let mut lines = text.lines().peekable();
    let mut fence = None;
    while let Some(line) = lines.next() {
        let trim = line.trim_start();
        if trim.starts_with("```") || trim.starts_with("~~~") {
            let c = trim.as_bytes()[0];
            if fence == Some(c) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(c);
            }
            continue;
        }
        if fence.is_some()
            || !lines
                .peek()
                .is_some_and(|next| Preview::table_header(line, next))
        {
            continue;
        }
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
            .is_some_and(|l| l.contains('|') && !l.trim_start().starts_with('#'))
        {
            let cells = Preview::cells(lines.next().unwrap());
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
}
impl Form {
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
    fn show(&self, h: H) {
        let q = &self.questions[self.active];

        msg(h, 683, 0x184, 0, 0);
        for choice in &q.choices {
            msg(h, 683, 0x180, 0, wide(choice).as_ptr() as _);
        }
        if let Some(index) = q
            .choices
            .iter()
            .position(|c| c == &self.answers[self.active])
        {
            msg(h, 683, 0x186, index, 0);
        }
        set(h, 684, &self.answers[self.active]);
    }
}
impl Form {
    pub(super) fn open(h: H, questions: Vec<Question>) -> Self {
        let form = Self::load(questions, &get(h, 403));
        msg(h, 681, 0x184, 0, 0);
        for q in &form.questions {
            msg(h, 681, 0x180, 0, wide(&q.title).as_ptr() as _);
        }
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
            if let Some(choice) = self.questions[self.active].choices.get(index) {
                set(h, 684, choice);
            }
        }
    }
    pub(super) fn apply(mut self, h: H) {
        self.answers[self.active] = get(h, 684);
        let draft = get(h, 403);
        if self.answers != Self::load(self.questions.clone(), &draft).answers {
            set(h, 403, &self.merged(&draft));
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
