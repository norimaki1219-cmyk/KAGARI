"""Read visible job listings on supported public search pages."""
import concurrent.futures
import html
import json
import re
import sys
from urllib.parse import urljoin, urlsplit
from urllib.request import Request, urlopen


AI = re.compile(r"生成\s*AI|LLM|ChatGPT|AIエージェント|大規模言語モデル|画像生成|動画生成|プロンプト|Claude|Gemini", re.I)
PATHS = {"coconala.com": r"/requests/\d+", "freelance.findy-code.io": r"/works/[\w-]+"}


def fetch(url):
    with urlopen(Request(url, headers={"User-Agent": "KAGARI/1.0"}), timeout=12) as response:
        return response.read(6_000_000).decode("utf-8")


def plain(value):
    return " ".join(html.unescape(re.sub(r"<[^>]+>", " ", value)).split())


def detail(url):
    page = fetch(url)
    if urlsplit(url).hostname == "coconala.com":
        visible = plain(re.sub(r"<(script|style)\b[^>]*>.*?</\1>", "", page, flags=re.S | re.I))
        terms = re.search(r"予算 (.{1,100}?) 納品希望日 .*? 募集期限 あと (.{1,35}?) 締切日 (\d+年\d+月\d+日)", visible)
        if not terms:
            return None
        title = plain(re.search(r"<h1[^>]*>(.*?)</h1>", page, re.S)[1])
        body = visible.split("募集内容 募集内容 ", 1)
        if len(body) != 2:
            raise ValueError("募集内容を確認できません")
        description = re.split(r"添付ファイル|応募者一覧|募集者情報", body[1])[0]
        conditions = "予算 " + terms[1] + " ／ 応募締切 " + terms[3]
        relevant = title + description[:1600]
    else:
        data = re.search(r'<script id="__NEXT_DATA__"[^>]*>(.*?)</script>', page, re.S)
        work = json.loads(data[1])["props"]["pageProps"]["initialWork"]
        if not work["isOpened"]:
            return None
        title = work["title"]
        description = work["jobContent"] or work["overview"] or ""
        relevant = title + description + (work["overview"] or "")
        conditions = "月額上限 {:,}円".format(work["maxMonthlyWage"]) if work["maxMonthlyWage"] else "報酬は募集詳細を参照"
    if AI.search(relevant):
        return dict(url=url, title=title, summary=plain(description)[:240], jobBody=description.strip(), significance=conditions)
    return None


def main(sources):
    items, checked, errors, details = [], [], [], []
    for source in sources:
        host = urlsplit(source).hostname
        if host not in PATHS:
            details.append(source + "　Dots対象")
            continue
        before = len(items)
        try:
            page = fetch(source)
            links = (urljoin(source, html.unescape(v)) for v in re.findall(r'href="([^"]+)"', page))
            urls = sorted({u.split("?")[0].split("#")[0] for u in links if urlsplit(u).hostname == host and re.fullmatch(PATHS[host], urlsplit(u).path)})
            if not urls:
                raise ValueError("案件リンクを確認できません")
            failed = 0
            with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
                for future in concurrent.futures.as_completed([pool.submit(detail, url) for url in urls]):
                    try:
                        item = future.result()
                        if item:
                            items.append(dict(item, source=source))
                    except Exception:
                        failed += 1
            if failed:
                errors.append(host + " 詳細取得失敗 " + str(failed) + "件")
            else:
                checked.append(source)
            details.append(source + "　取得 " + str(len(items)-before) + "件" + (" ／ 詳細取得失敗 " + str(failed) + "件" if failed else ""))
        except Exception as exc:
            errors.append(host + " " + str(exc))
            details.append(source + "　取得失敗 " + str(exc))
    return dict(items=items, checkedSources=checked, errors=errors, details=details)


if __name__ == "__main__":
    print(json.dumps(main(json.load(sys.stdin)["sources"])))
