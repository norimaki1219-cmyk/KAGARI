use super::*;

#[derive(Default)]
pub(super) struct Preview {
    pub(super) rtf: String,
    visible: String,
    offset: i32,
    has_tables: bool,
    headings: Vec<(usize, String)>,
    pub links: Vec<(i32, i32, String)>,
}

#[repr(C)]
struct Find {
    from: i32,
    to: i32,
    needle: Ptr,
    start: i32,
    end: i32,
}

impl Preview {
    fn text(&mut self, s: &str) {
        self.rtf.push_str(&rtf_text(s));
        self.visible.push_str(s);
        self.offset += s.encode_utf16().count() as i32;
    }

    fn inline(&mut self, mut s: &str) {
        while !s.is_empty() {
            if s.starts_with('[') {
                if let Some((len, target)) = reply_links::token(s) {
                    if let Some((label, _)) = s[1..len].split_once("](") {
                        let start = self.offset;
                        self.text(label);
                        self.links.push((start, self.offset, target.to_owned()));
                        s = &s[len..];
                        continue;
                    }
                }
            }
            let mut matched = false;
            for (marker, format) in [
                ("~~", r"\strike "),
                ("**", r"\b "),
                ("__", r"\b "),
                ("`", r"\f1 "),
                ("*", r"\i "),
            ] {
                if let Some(rest) = s.strip_prefix(marker) {
                    if let Some((body, tail)) = rest.split_once(marker) {
                        if !body.is_empty() {
                            self.rtf.push('{');
                            self.rtf.push_str(format);
                            if marker=="`"{self.text(body);}else{self.inline(body);}
                            self.rtf.push('}');
                            s = tail;
                            matched = true;
                            break;
                        }
                    }
                }
            }
            if matched {
                continue;
            }
            let first = s.chars().next().unwrap().len_utf8();
            let end = s[first..]
                .find(['[', '~', '*', '_', '`'])
                .map_or(s.len(), |i| i + first);
            self.text(&s[..end]);
            s = &s[end..];
        }
    }

    pub(super) fn cells(line: &str) -> Vec<String> {
        let body = line.trim().strip_prefix('|').unwrap_or(line.trim());
        let body = body.strip_suffix('|').unwrap_or(body);
        let mut cells = Vec::new();
        let mut cell = String::new();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' && chars.peek() == Some(&'|') {
                cell.push(chars.next().unwrap());
            } else if c == '|' {
                cells.push(cell.trim().to_owned());
                cell.clear();
            } else {
                cell.push(c);
            }
        }
        cells.push(cell.trim().to_owned());
        cells
    }

    pub(super) fn table_header(header: &str, divider: &str) -> bool {
        if !header.contains('|') || !divider.contains('|') {
            return false;
        }
        let cols = Self::cells(divider);
        cols.len() == Self::cells(header).len()
            && cols.iter().all(|s| {
                let rule = s.trim_matches(':');
                !rule.is_empty() && rule.bytes().all(|b| b == b'-')
            })
    }

    fn table(&mut self, rows: &[Vec<String>], width: i32) {
        let mut weights = vec![5; rows[0].len()];
        for row in rows {
            for (w, cell) in weights.iter_mut().zip(row) {
                let len = cell
                    .chars()
                    .map(|c| if c.is_ascii() { 1 } else { 2 })
                    .sum::<i32>();
                *w = (*w).max(len.min(42));
            }
        }
        let total: i32 = weights.iter().sum();
        for (index, row) in rows.iter().enumerate() {
            self.rtf.push_str(r"\trowd\trgaph60\trleft0 ");
            let mut sum = 0;
            for w in &weights {
                sum += w;
                self.rtf.push_str(r"\clbrdrb\brdrs\brdrw10\brdrcf2 ");
                if index == 0 {
                    self.rtf.push_str(r"\clcbpat3 ");
                }
                self.rtf.push_str(&format!(
                    r"\cellx{} ",
                    i64::from(width) * i64::from(sum) / i64::from(total)
                ));
            }
            self.rtf.push_str(r"\pard\intbl ");
            for col in 0..weights.len() {
                self.rtf.push('{');
                if index == 0 {
                    self.rtf.push_str(r"\cf1\b ");
                }
                if let Some(cell) = row.get(col) {
                    self.inline(cell);
                }
                self.visible.push('\n');
                self.offset += 1;
                self.rtf.push_str(r"}\cell ");
            }
            self.rtf.push_str(r"\row ");
        }
        self.rtf.push_str(r"\pard ");
    }

    pub fn document(s: &str, width: i32) -> Self {
        Self::document_with_checks(s, width, "")
    }

    pub(super) fn document_with_checks(s: &str, width: i32, marker: &str) -> Self {
        let mut out = Self::default();
        out.rtf.push_str(r"{\rtf1\ansi\uc1\deff0{\fonttbl{\f0 Yu Gothic UI;}{\f1 Consolas;}}{\colortbl;\red16\green100\blue117;\red182\green205\blue210;\red226\green241\blue243;}\f0\fs24 ");
        let mut fenced = None;
        let mut first = true;
        let mut lines = s.trim_start_matches('\u{feff}').lines().peekable();
        while let Some(line) = lines.next() {
            let trim = line.trim_start();
            let fence = trim.chars().next().filter(|c| matches!(c, '`' | '~'));
            if let Some(c) = fence {
                let size = trim.chars().take_while(|v| *v == c).count();
                if size >= 3 && fenced.is_none() {
                    fenced = Some((c, size));
                    continue;
                }
                if fenced.is_some_and(|(ch, n)| ch == c && size >= n)
                    && trim[size..].trim().is_empty()
                {
                    fenced = None;
                    continue;
                }
            }
            if !first {
                out.rtf.push_str(r"\par ");
                out.visible.push('\n');
                out.offset += 1;
            }
            first = false;
            {
                if fenced.is_none()
                    && lines
                        .peek()
                        .is_some_and(|next| Self::table_header(line, next))
                {
                    lines.next();
                    let mut rows = vec![Self::cells(line)];
                    while lines.peek().is_some_and(|next| {
                        next.contains('|') && !next.trim_start().starts_with('#')
                    }) {
                        rows.push(Self::cells(lines.next().unwrap()));
                    }
                    out.has_tables = true;
                    out.table(&rows, width);
                    first = true;
                    continue;
                }
            }
            out.rtf.push('{');
            let heading = trim.bytes().take_while(|b| *b == b'#').count();
            if fenced.is_some() {
                out.rtf.push_str(r"\f1 ");
                out.text(line);
            } else if (1..=6).contains(&heading) && trim[heading..].starts_with(' ') {
                let size = [34,32,30,28,27,26][heading-1];
                out.rtf.push_str(&format!(r"\cf1\b\fs{size} "));
                let start = out.visible.len();
                out.inline(trim[heading..].trim_start());
                out.headings
                    .push((heading, out.visible[start..].to_owned()));
            } else if let Some(body) = trim
                .strip_prefix("- ")
                .or_else(|| trim.strip_prefix("* "))
                .or_else(|| trim.strip_prefix("+ "))
            {
                out.text(&line[..line.len() - trim.len()]);
                let (bullet, body) = if let Some(body) = body.strip_prefix("[x] ").or_else(|| body.strip_prefix("[X] ")) {
                    ("☑ ", body)
                } else if let Some(body) = body.strip_prefix("[ ] ") {
                    ("□ ", body)
                } else {
                    ("• ", body)
                };
                if !marker.is_empty() && bullet != "• " {
                    // The byte between the brackets is the only byte changed on click.
                    let offset = trim.as_ptr() as usize - s.as_ptr() as usize + 3;
                    out.text(&format!("{marker}{offset}:"));
                } else {
                    out.text(bullet);
                }
                out.inline(body);
            } else if let Some(body) = trim.strip_prefix("> ") {
                out.rtf.push_str(r"\cf1\i ");
                out.text("│ ");
                out.inline(body);
            } else {
                out.inline(line);
            }
            out.rtf.push('}');
        }
        out.rtf.push('}');
        let explicit_links = out.links.len();
        reply_links::links(&out.visible, |start, end, target| {
            if !out.links[..explicit_links].iter().any(|(a,b,_)| start < *b && *a < end) {
                out.links.push((start, end, target.to_owned()));
            }
        });
        out
    }

    pub fn show_answer(mut self, h: H, id: i32) -> Vec<(i32, i32, String)> {
        self.show(h, id);
        if self.has_tables && !self.links.is_empty() {
            // Rich Edit represents table boundaries with additional native characters.
            self.links.sort_by_key(|r| r.0);
            let mut mapped = Vec::new();
            let mut next = 0;
            let mut offset = 0;
            let mut native_from = 0;
            for line in self.visible.split('\n') {
                let end = offset + line.encode_utf16().count() as i32;
                if !line.is_empty() {
                    let needle = wide(line);
                    let mut hit = Find {
                        from: native_from,
                        to: -1,
                        needle: needle.as_ptr(),
                        start: 0,
                        end: 0,
                    };
                    if msg(h, id, 0x47c, 1, &mut hit as *mut _ as isize) >= 0 {
                        native_from = hit.end;
                        while next < self.links.len() && self.links[next].0 < end {
                            let (a, b, target) = &self.links[next];
                            if *a >= offset && *b <= end {
                                mapped.push((
                                    hit.start + a - offset,
                                    hit.start + b - offset,
                                    target.clone(),
                                ));
                            }
                            next += 1;
                        }
                    }
                }
                offset = end + 1;
            }
            self.links = mapped;
        }
        reply_links::style_ranges(h, id, &self.links);
        self.links
    }

    pub fn select_source_line(h:H,id:i32,source:&str,start:i32,end:i32){
        let normalized=source.replace("\r\n","\n").replace('\r',"\n");
        let units:Vec<u16>=normalized.encode_utf16().collect();
        if start<0 || end<=start || end as usize>units.len(){return;}
        let before=String::from_utf16_lossy(&units[..start as usize]);
        let through=String::from_utf16_lossy(&units[..end as usize]);
        let width=native::document_width(h,id);
        let prior=Self::document(before.trim_end_matches('\n'),width);
        let current=Self::document(&through,width);
        let previous=prior.visible.trim_end_matches('\n');
        let selected=current.visible.strip_prefix(previous).unwrap_or(&current.visible);
        let mut cursor=0;
        let mut first=None;
        for (part,select) in [(previous,false),(selected,true)]{
            for text in part.lines().map(str::trim).filter(|s|!s.is_empty()){
                let needle=wide(text);
                let mut hit=Find{from:cursor,to:-1,needle:needle.as_ptr(),start:0,end:0};
                if msg(h,id,0x47c,1,&mut hit as *mut _ as isize)>=0{
                    cursor=hit.end;if select{first.get_or_insert(hit.start);}
                }
            }
        }
        if let Some(start)=first{
            msg(h,id,0xb1,start as usize,cursor as isize);msg(h,id,0xb7,0,0);
        }
    }

    pub fn populate_contents(&self, h: H) {
        if unsafe { GetDlgItem(h, 672) } == 0 {
            return;
        }
        #[link(name="user32")]
        extern "system" {
            fn GetDC(hwnd:H)->H;
            fn ReleaseDC(hwnd:H,dc:H)->i32;
        }
        #[link(name="gdi32")]
        extern "system" {
            fn SelectObject(dc:H,object:H)->H;
            fn GetTextExtentPoint32W(dc:H,text:Ptr,len:i32,size:*mut [i32;2])->i32;
        }
        let list=unsafe{GetDlgItem(h,672)};
        let dc=unsafe{GetDC(list)};
        let font=msg(h,672,0x31,0,0);
        let previous=unsafe{SelectObject(dc,font)};
        let mut extent=0;
        let mut from = 0;
        'heading: for (level, label) in &self.headings {
            if label.is_empty() {
                continue;
            }
            let needle = wide(label);
            let mut find = Find {
                from,
                to: -1,
                needle: needle.as_ptr(),
                start: 0,
                end: 0,
            };
            // Query Rich Edit for its native character positions, including table cells.
            loop {
                if msg(h, 120, 0x47c, 1, &mut find as *mut _ as isize) < 0 {
                    continue 'heading;
                }
                if reply_links::text_height(h, 120, find.start) > 240 {
                    break;
                }
                find.from = find.end;
            }
            let item = wide(&format!("{}{}", "　".repeat(level - 1), label));
            let mut size=[0;2];
            unsafe{GetTextExtentPoint32W(dc,item.as_ptr(),(item.len()-1) as i32,&mut size);}
            extent=extent.max(size[0]+8);
            let index = msg(h, 672, 0x180, 0, item.as_ptr() as _);
            msg(h, 672, 0x19a, index as usize, find.start as isize);
            from = find.end;
        }
        msg(h, 120, 0xb1, 0, 0);
        msg(h, 120, 0xb7, 0, 0);
        unsafe{SelectObject(dc,previous);ReleaseDC(list,dc);}
        msg(h,672,0x194,extent as usize,0); // LB_SETHORIZONTALEXTENT
        let count = msg(h, 672, 0x18b, 0, 0);
        if count > 0 {
            msg(h, 672, 0x186, 0, 0);
        } else {
            msg(h, 672, 0x180, 0, wide("見出しなし").as_ptr() as _);
            msg(h, 672, 0x186, 0, 0);
        }
        unsafe {
            EnableWindow(GetDlgItem(h, 672), (count > 0) as i32);
        }
    }

    pub fn show(&self, h: H, id: i32) {
        let rtf = std::ffi::CString::new(self.rtf.as_str()).unwrap();
        let params = [0u32, 1252];
        msg(h, id, 0x461, params.as_ptr() as usize, rtf.as_ptr() as _);
    }
}

#[cfg(test)]
mod answer_link_tests {
    use super::*;
    #[test]
    fn links_bare_url_in_japanese_reply_after_table() {
        let url = "https://eda.eda-edamap.workers.dev";
        let answer = format!("| A | B |\n|---|---|\n| 1 | 2 |\n\n- 利用者：本番 {url} で、ログインしてください。");
        let p = Preview::document(&answer, 6000);
        let start = p.visible.split_once(url).unwrap().0.encode_utf16().count() as i32;
        assert_eq!(p.links, vec![(start,start+url.len() as i32,url.to_owned())]);
        let p = Preview::document("（https://example.org/a(b)?q=1&k=2）。",6000);
        assert_eq!(p.links[0].2,"https://example.org/a(b)?q=1&k=2");
        let p = Preview::document("[https://example.org](https://example.org/actual)",6000);
        assert_eq!(p.links.len(),1);
        assert_eq!(p.links[0].2,"https://example.org/actual");
    }
    #[test]
    fn renders_slack_and_numbered_document_links_as_clickable_labels() {
        let url="https://example.slack.com/archives/C123/p456";
        let p=Preview::document(&format!("🙂 **[受領返信]({url})**・[記録](D:/work/CLAUDE.md:214)"),6000);
        assert_eq!(p.visible,"🙂 受領返信・記録");
        assert_eq!(p.links,vec![(3,7,url.to_owned()),(8,10,"D:/work/CLAUDE.md:214".to_owned())]);
        assert!(Preview::document("[無効](file:///example.exe)",6000).links.is_empty());
    }
}
