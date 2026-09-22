# MVP検証結果

2026-09-22 / Windows 11 x64 / Flutter 3.47.0 / Dart 3.13.0 / Rust 1.97.1。

## 確認済み

- Rust回帰テスト11件成功。決定的なPNGバイト列、alpha 0/255、透明RGBの正規化、パレット上限、PNG/JPEG/WebP、Median、入力拒否、非整除寸法、孤立面ノイズ、少数派の連続線、斜線・窓枠、FFIメモリ所有権／エラー応答を検証。
- Autoグリッド: 32×32参照画像の3/4/6/8倍、および197×197・1254×1254への非整数再拡大を検証。
- `cargo clippy --all-targets -- -D warnings`: 指摘なし。
- `flutter analyze`: 指摘なし。
- Flutter widgetテスト: 390×844 / 1280×800、初期操作の無効化、レイアウトoverflowなし。
- Windows統合テスト: 画像読込、FFI/isolateでの変換、32×32結果、PNGの再現性、ファイル書込、同一サイズのBefore/After枠、設定変更による結果無効化。2026-09-23に両側からのホイール拡大・ドラッグ移動で描画変換が一致すること、および変換前の拡大位置が変換後にも引き継がれることを追加検証。
- Windows x64 Releaseビルド成功。native assetとして`pixelstrict_core.dll`の同梱を確認。
- Windows UI: 実行ファイルの起動引数からの画像読込、nativeファイル選択、変換、Before/After、処理時間、native保存ダイアログ、保存成功表示を確認。保存PNGのSHA-256はCLIサンプルと一致。最終ビルドで両プレビューが同じ表示サイズになることも確認。
- CLI: 実ファイルの変換、JSON report、既存出力を上書きしない動作を確認。
- Android ARM64 debug APKビルド成功。APK内の`lib/arm64-v8a/libpixelstrict_core.so`、minSdk 24、targetSdk 36を確認。
- macOS ARM64 / iOS ARM64向けRust coreの`cargo check`成功。リンク／Flutter runnerの実行確認ではありません。

## アイコン更新（2026-09-23）

- WindowsのRelease EXE内の9サイズ（16〜256px）のアイコンが新しいICOの画像データと完全一致することを検証。ウィンドウも同じリソースを参照。
- iOS / macOSのAppIcon全29エントリの寸法を検証。iOSは透過のないRGB。Apple向けFlutterビルドは未実施。
- Android ARM64 debug APKを再ビルド。adaptive / monochromeアイコンとFlutter内ロゴの格納を確認。端末での表示は未検証。
- Flutter静的解析、widgetテスト、Windows FFI統合テスト成功。Windows Releaseを再ビルドし、配布ZIP内の実行ファイル・ロゴが最新の成果物と一致することを確認。

## 比較結果

192×192合成画像（315色）→32×32、Auto 11色。最新の計測は約27ms。手動Grid 32・Colors 32では平滑化弱22色／強16色、約2〜3ms。参照とのRGB平均絶対誤差は弱3.09／強2.89（0〜255値）。1254×1254への非整数拡大では32×32を自動選択、約109ms。単発のローカル測定であり、端末性能・並行ビルドの影響を含みます。

`samples/comparison.png`と各report JSONを参照してください。合成入力に対する結果で、一般のAI生成画像での品質保証ではありません。

## 残る確認事項・外部警告

- Android実機、macOS/iOSのFlutterビルド・署名・実機操作は未検証。Android APKはdebug署名の検証用。
- Desktop D&Dはdesktop_drop経由で実装。OSからの実ドラッグ操作は未検証。
- desktop_drop 0.8.4が旧Kotlin Gradle Pluginを使用するため、将来のFlutterに対する移行警告が出ます。依存側の対応が必要です。現在のAPKビルドは成功。
- 初回AndroidビルドでローカルSDK XMLバージョン差の警告。SDKツール更新が必要です。
- RustビルドがMSVCの日本語「インポートライブラリ作成」メッセージを`linker_messages`警告として表示。リンク成功済みで、コード不良ではありません。
- Webの成果物はなく、PSI/Lighthouse/Chrome CDP Issuesの対象URLなし。
- Gitリポジトリ／remoteは元から未設定。commit・push・外部公開は行っていません。
