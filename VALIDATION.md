# 検証結果

## Auto Gridの線幅保護（2026-09-23、Shape protection追加後）

線の中心と背景が残っていれば、線が太くなっても連続性の保持と判定されていた問題を修正。既存の元画像プローブに線幅を記録し、候補の実出力と元画像座標で比較する`line_width_retention`を追加しました。shape scoreの線項目とalignment加点に反映し、分類・色決定・描画・C ABI・設定UIは変更していません。新規依存なし。

- `line-heavy`のAutoは**12×12セル（P=4）→48×48セル（P=1）**。1px線の太さと位置を保持し、参照と異なる画素数は566→0。1/2/4倍、斜線の向き違い、Preserve / Logicalの両方を全画素比較しています。
- Preserveは元寸法・整数セル・単色矩形を維持。Logicalは元画像上の整数区間で幅を評価し、非整除の横長／縦長入力でも座標対応を確認。補間・anti-aliasingは追加していません。
- 既存houseのAutoは32×32・11色、Manualは弱24色／強20色。3種類のPNGは直前版`672740c`とバイト単位で一致。building-like / flat-with-noiseとManualの線fixtureもPNGは不変。JSON reportと比較資料を再生成しました。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **31件成功**。追加3件は線幅の拡大／縮小／消失、非整除座標、Autoの幅と位置。既存の外形・細線・小形状・単独ノイズ・単色セル・両出力モード・決定論性の回帰も成功。
- `cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`cargo fmt --manifest-path core/Cargo.toml --check`、`flutter analyze --no-pub`: **成功、指摘なし**。
- Windows FFI統合テストと`flutter build windows --release --no-pub`: **成功**。全候補の追加JSON指標を読み取り、preview・保存・設定変更後の再変換を確認。Release同梱DLLを直接C ABIで呼び、Autoの採用グリッド・追加指標・生成PNGが最新fixtureと一致することも検証。

旧／新のWindows Release同梱DLLを同じプロセスでwarm-up後に各5回、実行順を交互にして計測した中央値（decode〜PNG encode）:

| 入力・設定 | 直前版`672740c` | 線幅保護追加後 |
| --- | ---: | ---: |
| 192×192・Auto Logical | 40.4ms | 40.0ms |
| 1254×1254・Auto Logical | 152.6ms | 148.8ms |
| 1254×1254・Preserve Grid 418・Colors 24・Smoothing 3 | 209.6ms | 209.2ms |

入力は同じhouse画像で、1254版はNearest Neighborによる拡大。この範囲で目立つ追加コストはなく、差はローカル計測の変動範囲です。先のShape protection全体による元MVP比のコスト増は引き続き存在します。

線幅探索は各方向64セルまでで、広い線の端が上限内に見つからない場合は過剰な細線化ペナルティを避ける近似です。太い線の境界には元幅の25%（整数切り捨て）の許容差を設け、1px線は太さの増加を許容しません。接触・交差した線、量子化前に失われる低コントラスト細部、候補集合にないグリッドは今後の課題です。実際の建物画像での評価、Android / macOS / iOSの再ビルド・実機検証は未実施。以下は各変更時点の履歴で、直下のline-heavyの線幅制限は今回改善済みです。

## Shape protection追加（2026-09-23）

Windows 11 x64 / Flutter 3.47.0 / Dart 3.13.0 / Rust 1.97.1。Rust core中心の変更で、依存追加なし。preprocess → quantize → grid candidate → analyze → decide → evaluate、C ABI、両出力モードを維持しています。以下より下の記録は各変更時点の履歴です。

- 外形候補、LINE（1〜2セル幅・8方向・端点・曲がり角）、2〜6セルの連結小形状を追加。FLAT起点の統合が線や小形状を消す問題を修正。外形付近の色統合は通常より狭い許容差に制限。
- 領域別effective smoothing、形状ボーナス、実出力の接続評価を追加。元画像の共通プローブは最大16,384件で、粗い候補から線／小形状が消えても保持率を過大評価しない構成。保持率が低い候補のalignment加点も抑制。
- UIにShape protection 弱／中／強、CLIに`--shape`、Options JSONに`shape_protection`を追加。未指定は2。Reportは既存classesの先頭4位置を保ちLINEを末尾へ追加し、外形・保持した小形状のセル数と形状指標を追加。C ABIの関数・バッファ所有権は変更なし。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **28件成功**（既存19件＋Shape回帰9件）。透明外形・突起、透明／不透明背景の少数派線の端点、1〜2セル幅の縦横／両斜線、窓枠の角、単独ノイズ、2〜6セル形状、形が消える粗い候補の低評価、Auto、決定論的PNGを確認。
- Preserveの1254×1254・P=2 / 3を含む既存全画素テストに成功。均等な整数単色矩形、元寸法、alpha 0/255、透明RGB=0、Logicalとの各セル色一致を維持。Logicalの非整除分割・既知32×32グリッドの3/4/6/8倍と197/1254への再拡大も継続して成功。
- 新アルゴリズムの出力に合わせて既存PNG goldenとReportを再生成。Goldenとのバイト一致に加え、独立した32×32参照画像とのRGB MAE < 3.5も検証。旧版のPNG自体とのバイト一致は今回の仕様では要求せず、同じ入力・設定での再現性を維持。
- `cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`cargo fmt --manifest-path core/Cargo.toml --check`: **成功、指摘なし**。
- `flutter analyze --no-pub`: **指摘なし**。`flutter test --no-pub`: **成功**。390×844 / 1280×800のShape初期値・変更、Output切替、操作の無効化とoverflowなしを確認。
- Windows FFI統合テスト: **成功**。Shape 2/3のJSON受け渡し、追加Report、Preserve/Logicalの実PNG寸法、再現性、保存バイト列、Shape変更後の保存無効化、同期ズーム／パンと再変換後の位置維持を確認。
- `flutter build windows --release --no-pub`: **成功**。同梱された`pixelstrict_core.dll`を直接C ABIで呼び、Shape未指定=2、classes 5要素、Autoでの小形状保持、最新PNG goldenとの一致を確認。成果物は`app/build/windows/x64/runner/Release/`。既存MVP ZIP / APKは更新せず、`output/`はGit対象に含めていません。

### 合成fixtureでの比較

`samples/shape/`にbuilding-like / line-heavy / flat-with-noiseの入力、Manual / AutoのPNGとJSONを追加。同じ解像度・Grid 48・Colors 32・Smoothing 3・Edge 2で旧版`9f081f1`と比較しました。

| 項目 | 旧版 | Shape中 |
| --- | ---: | ---: |
| 低コントラストの建物の縁・窓枠174セル | 0保持 | 174保持 |
| 2×3セルの小形状 | 0保持 | 6保持 |
| 単独面ノイズ5点 | 5除去 | 5除去 |

Autoではbuilding-likeとflat-with-noiseが48×48セル、line-heavyが12×12セル（P=4）を採用。線主体のAutoは連続性を保つ一方で線幅が太くなるケースが残ります。Manualの建物の角・斜線・小形状を視覚比較し、建物／ノイズfixtureのノイズ以外の出力は参照と一致することをテストでも検証しました。比較は合成入力についての結果で、実際のAI生成画像全般に対する保証ではありません。

既存houseのAutoは32×32・11色、参照RGB MAEは旧3.149→3.113。Colors 32のManualは弱24色／強20色、MAEは3.255／3.258（旧3.089／2.894）。形と見なした小さな色のまとまりも残すため、面の均一性やRGB誤差が必ず改善するわけではありません。今回の単発Release計測はAuto約45ms、Manual約4ms。元画像の形状走査とセルの連結解析が増えるため、旧版より処理コストは増加します。

旧／新のWindows Release同梱DLLを同じプロセスから各3回呼んだ中央値（decode〜PNG encode、ローカル計測）:

| 入力・設定 | 旧版 | 新版 |
| --- | ---: | ---: |
| 192×192・Auto Logical | 22.6ms | 38.4ms |
| 1254×1254・Auto Logical | 97.4ms | 146.8ms |
| 1254×1254・Preserve Grid 418・Colors 24・Smoothing 3 | 114.0ms | 203.3ms |

Auto Logicalはいずれも32×32を維持。入力は同じhouse画像で、1254版はNearest Neighborで拡大したもの。約1.5〜1.8倍のコスト増があり、極端に大きい画像・セル数での実機計測は引き続き必要です。プローブ数は上限固定、保護マスクはセル当たり3個の`u32`、連結解析は再利用する作業配列で実装しています。

### 残る制限・次の検証

- 外形判定は外周接続・透明境界・色差による近似で、前景／背景の意味分割ではありません。密な植物、似た色の接触物体、規則的なノイズ塊は誤保護の可能性があります。
- Medianまたは量子化で失われた細部は後段では復元できません。1セル1色より細かい形は選択するGridに制約されます。特に低コントラストの斜線や曲線のAuto評価は近似です。
- 次は実際の建物画像をfixture化して誤消去／誤保護を測定し、量子化前の形状情報の引き継ぎとbuilding / character / iconの係数調整を行うと効果的です。今回は係数を`ShapeBalance`に分離する土台まで実装。
- Android / macOS / iOSの再ビルド・実機操作は今回未実施。既知のMSVC `linker_messages`はインポートライブラリ作成の日本語出力によるもので、リンク・Clippyは成功。
- ネイティブアプリのためWeb成果物・監査対象URLなし。PSI / Lighthouse / Chrome CDP Issuesは対象外。GitHub Actions workflow / Deployment登録はいずれも0件で、自動デプロイ確認の対象なし。

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
