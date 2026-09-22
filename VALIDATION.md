# 検証結果

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
- 初回MVP検証時はGitリポジトリ／remoteが未設定で、commit・push・外部公開は行っていませんでした。

## Preserve Resolution追加（2026-09-23）

Windows x64 / Flutter 3.47.0 / Dart 3.13.0 / Rust 1.97.1。新規依存なし。preprocess・OKLab量子化・alignment・analyze / decide / evaluate・Surface Strict / Edge Strictの処理は維持。

- `cargo test --locked --manifest-path core/Cargo.toml`: **19件成功**（既存回帰11件＋出力モード7件＋CLI 1件）。従来の非整除グリッド・画質回帰はLogicalを明示して継続。
- Preserveの1254×1254・P=3（418×418セル）とP=2（627×627セル）で入力／出力寸法が完全一致。長方形・P=1 / 6 / 11も検証。全セル領域の全画素が同一RGBA、出力パレット以外の色なし、alphaは0/255のみ、透明はRGBA=(0,0,0,0)。同一入力・設定でPNGバイト列一致。上限内ではLogicalの各決定セル色とも完全一致。
- PreserveのAuto候補が両辺のgcdの全約数から生成されること、128pxを超えるpitchも含むこと、採用・評価対象が均等な正方形整数セルのみであることを検証。既存合成画像の3 / 4 / 6 / 8倍では32×32セルを維持。
- 1254×1254・Manual Grid 314、および幅のみ整除でき高さが整除できないケースは明確なエラー。Preserveは1,048,576セル、Logicalは従来の262,144セル上限を検証。両辺が互いに素で唯一のP=1が上限を超えるAuto入力もエラー。
- Logicalは従来の出力寸法・非整除分割を維持。既存Auto / Surface弱 / Surface強の3サンプルPNGと**バイト単位で一致**。
- 既存Options JSON（output_mode未指定）をC ABI経由で変換し、Preserveがデフォルトであること、追加Report項目と実PNG寸法を確認。C ABIの関数・所有権に変更なし。
- CLIは省略時Preserve、`--output preserve|logical`、既存オプションとの併用、help、JSON report、不正値／値欠落／非整除Gridの拒否、既存ファイルの上書き防止を検証。
- `cargo clippy --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`: **指摘なし**。
- `flutter analyze --no-pub`: **指摘なし**。`flutter test --no-pub`: **成功**。390×844 / 1280×800でOutputの初期値・切替・初期操作の無効化を確認。検証中に発見したOutput選択欄の横はみ出しは修正し、再検証でoverflowなし。
- `flutter test integration_test/conversion_test.dart -d windows --no-pub --dart-define=SAMPLE_PATH=C:/Users/Daiki/Documents/PixelStrict/pixel-strict/samples/house-pseudo.png`: **成功**。Preserveの192×192 PNG（32×32セル・6px）を実画像ヘッダとConversionResultの両方で検証。Logicalの32×32、保存バイト列、再現性、モード変更による古い結果の無効化、Manualのpitch表示・非整除エラー表示、両側の同期ズーム／パン、再変換後の表示位置維持も確認。
- `cargo build --release --locked --manifest-path core/Cargo.toml`、`flutter build windows --release --no-pub`: **成功**。Release CLIで1254×1254・Grid 418 / 627 / AutoのPreserve、Grid 418のLogical、Grid 314の拒否を確認。Windows Releaseに同梱された`pixelstrict_core.dll`も直接C ABIを呼び出し、未指定時Preserve・実PNG 192×192・pitch 6を確認。
- Windows成果物: `app/build/windows/x64/runner/Release/`、CLI: `core/target/release/pixelstrict.exe`。既存`output/`のMVP ZIP / APKは更新せず、Git対象にも含めていません。

今回の範囲で未解決のテスト失敗なし。MSVCの日本語インポートライブラリ作成メッセージがRustの`linker_messages`警告として出る既知の環境要因は継続（リンク成功、clippy指摘なし）。Android / macOS / iOSの再ビルド・実機検証は今回未実施。Web成果物・対象URLはなく、PSI / Lighthouse / Chrome CDP Issuesは対象外。GitHub Actions workflowとDeployment登録はともに0件のため、自動デプロイの検証対象なし。
