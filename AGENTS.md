# Windows の検証プログラム

KAGARI の検証用に作成した Check.exe、QueueCheck.exe などは、リポジトリ直下の `run-check.ps1 -Executable <検証用exe>` 経由で起動する。終了コードと標準エラーを確認し、失敗時は例外の原因を直してから再実行する。

WPF の非同期 UI を検証する場合は STA の Dispatcher 上で初期化、イベント実行、終了を行う。実運用のタスクと分けた作業フォルダーを使う。
