# SHIRANUI アラート管理 UI 改善パッケージ

## 内容

- `index.html` : ブラウザで開けるレスポンシブUIモック（ローカルファイルで表示可能）
- `assets/icons/*.svg` : 27種の軽量なモノクロ線画アイコン。`currentColor`で着色可能
- `assets/design-tokens.json` : 配色、余白、文字サイズ、レイアウト定数
- `docs/UI_IMPROVEMENT.md` : 現状課題、設計方針、コンポーネント仕様、導入優先順位
- `docs/IMPLEMENTATION_HANDOFF.md` : Codex / Claude Code / WPF / Rust への実装引き継ぎ
- `reference_original.png` : ご提供の元画面

## 使い方

1. ZIPを展開します。
2. `index.html` をブラウザで開いて、フィルタ、行選択、状態変更、CSV出力、タブ切替を試します。
3. SVGは`assets/icons`から利用し、色と寸法はトークンを基準に調整してください。

**重要:** モックは参考画面に基づくサンプルデータを含む静的デモです。SHIRANUIの実行状況、実検知、実判定、実キューには接続していません。参考画面の集計値と、一覧のデモレコード数は一致しません。実運用ではバックエンドの実データを使用してください。判定操作はモック内部だけに反映されます。
