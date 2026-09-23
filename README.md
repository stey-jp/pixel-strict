# PixelStrict

AI生成の疑似ピクセルアートを、整数グリッド・1セル1色のPNGへ再構成するローカルアプリです。Flutter UI / Rust core / C ABI FFI。OpenAI API・画像生成・OpenCVは使用しません。

Shape protection / Preserve対応のWindows Releaseは`flutter build windows --release`で生成する`app/build/windows/x64/runner/Release/`です。初回MVPのビルド: `output/PixelStrict-windows-x64.zip`、`output/PixelStrict-android-arm64-debug.apk`（今回未更新）。[検証結果](VALIDATION.md)と[比較画像](samples/comparison.png)も参照してください。Windows配布時はexe単体ではなくDLL・dataを含むReleaseフォルダー全体が必要です。

## 構成

```text
PixelStrict/
├─ app/          Flutter、手書きFFIバインディング、native asset build hook
├─ core/         UI非依存Rustライブラリ、Cヘッダ、CLI、回帰テスト
├─ samples/      再現可能な合成入力・参照画像・変換結果・比較
├─ scripts/      起動・検証・比較用スクリプト
└─ README.md
```

## Windowsで起動

必要環境: Flutter 3.47 / Dart 3.13以降、Rust 1.97.1、Visual Studio 2022の「C++によるデスクトップ開発」。Rustバージョンと依存lockfileを固定しています。

```powershell
./scripts/run-windows.ps1
# 起動引数で画像を開く
./scripts/run-windows.ps1 -ImagePath ./samples/house-pseudo.png
```

通常のFlutterコマンドでもビルドできます。native asset hookがRustをreleaseモードでビルドし、共有ライブラリを同梱します。

```powershell
cd app
flutter pub get
flutter run -d windows
flutter build windows --release
```

Developer Modeが無効でプラグインリンクの作成に失敗する場合、`scripts/windows-plugin-links.ps1`が**プロジェクト内だけ**にjunctionを作成します。その後は`--no-pub`を指定してください。OS設定は変更しません。

`core/rust-toolchain.toml`は全対応ターゲットを宣言しています。Rustupによる他OSの標準ライブラリ取得を避けたい場合は、使用OSのRust 1.97.1をインストール後、`RUSTUP_TOOLCHAIN=1.97.1`をプロセス環境変数に設定してください。実際のクロスビルドには対象の`rustup target add --toolchain 1.97.1 <target>`が必要です。

## 操作

1. PNG / JPEG / WebPを選択。Windows / macOSはD&Dに対応。
2. Output: **Preserve size（デフォルト）**、またはLogical pixelsを選択。
3. Grid: Auto、またはManualで**Grid width（横方向のセル数）**を指定。Preserveでは元画像の両辺を割り切る均等な正方形整数セルのみ使用し、Manualには`418 cells / 3px`のように表示します。Logicalでは従来どおり高さを縦横比から整数に丸めます。
4. Colors: Auto / 16 / 24 / 32。指定数は透明色を含む上限で、不要な色は追加しません。
5. Surface smoothing / Edge protection / **Shape protection**を弱・中・強から選択（初期値は中）。Shapeは外形・細線・小形状の保持を調整します。MedianはOff / 半径1px。
6. 変換後、Before / Afterを比較してPNGを保存。片方をズーム／パンすると、もう片方も同じ倍率・位置に同期します。変換・再変換時は表示位置を保持し、新しい画像を読み込むと全体表示に戻ります。プレビューの拡大表示はNearest Neighborのみ。

保存PNGは次の2モードから選択できます。

- **Preserve Resolution**（UI: Preserve size、デフォルト）: 元画像と完全に同じwidth / height。決定済みの論理セル色をRustで均等な`P×P`整数矩形へ直接描画します。縮小・拡大APIや補間は使いません。1254×1254・Grid 418なら、3×3pxの単色セルで1254×1254を保存します。
- **Logical Resolution**（UI: Logical pixels）: 従来の1セル＝出力1px。1254×1254・Grid 418なら418×418、192×192・Grid 32なら32×32を保存します。

Preserveでは`source_width % P == 0`かつ`source_height % P == 0`が必須です。1254×1254ではP=2 / 3 / 6 / 11（Grid 627 / 418 / 209 / 114）は有効、P=4やManual Grid 314はエラーです。不均等セルへの暗黙の変換は行いません。Autoも両辺の最大公約数の約数からpitchを選択します。セル数上限内の候補がない場合はエラーになり、Logicalへ切り替えられます。

入力alphaは128を閾値に0 / 255へ二値化し、透明画素はRGBA=(0,0,0,0)に統一します。アンチエイリアス・半透明境界・ディザリングなし。入力上限は64 MiB、各辺8192px、合計16,777,216画素。論理セル数上限はPreserveで1,048,576、Logicalでは従来どおり262,144です。

変換中はUIを止めず、操作をロックして結果の取り違えを防ぎます。設定変更時は古い結果の保存を無効化します。PNGには時刻・乱数などを埋め込まず、同じ実行環境・入力・設定で同一バイト列になります。

## 処理設計

1. ヘッダ段階で寸法と形式を検査し、制限付きでdecode。
2. アルファを二値化。任意の3×3 Medianは同じ不透明領域内だけに適用。
3. RGB各5bitの重み付きヒストグラムをOKLabへ変換。決定的なfarthest-point初期化＋8回のweighted k-meansで減色。ditheringなし。
4. 元画像からセル候補を生成。Preserveは`gcd(width, height)`の全約数を整数pixel pitch候補とし、均等な正方形セルとモード別セル数上限を満たす候補のみ使用します。Logicalは従来どおり非整除寸法も整数区間へ分割。どちらも全入力画素を1回ずつ所属させます。
5. 微小な色ノイズを抑えた横・縦のOKLab色差投影から境界整合度を求め、上位8候補＋可能なら原寸候補を精査。境界付近の±1pxを評価し、非整数拡大による位置の丸めを許容します。
6. 占有率・空間共分散・周囲8セルからFLAT / EDGE / DETAIL / UNKNOWN / **LINE**に分類。透明境界と外周につながる面の高コントラスト境界をsilhouette候補にします。1〜2セル幅の線・斜線・端点・曲がり角をLINEとして保護。近似色をまとめた8連結成分で2〜6セルの小形状を保護し、セル内の高コントラストな少数派色も連結性を確認します。
7. `占有率 + 周囲／エッジの連続性 + LINE／silhouette／小形状ボーナス − 孤立ペナルティ − 不要な色差`で決定。ユーザーのsmoothingにFLAT=1、EDGE=0.10、LINE=0.025、DETAIL=0.035、UNKNOWN=0.10、silhouette=0.015の領域係数を掛けます。単独の近似色ノイズは面へ統合します。
8. FLATを起点に固定seedとの色差を制限して面を再統合。LINEと小形状を多数決で消しません。silhouetteを統合の起点にせず、同じ面へ吸収できる色差も通常の20%までに制限します。透明／不透明や高コントラストな外形は統合できません。
9. 従来の面均一性・入力誤差・境界整合度に、外形保持・出力上の線の接続・小形状保持・角／境界整合性を追加。全候補で共通の元画像プローブ（最大16,384件）を参照するため、粗い候補でLINEや小形状の検出数がゼロになっても無条件に高得点にはなりません。形状の保持率が低い場合は境界整合度の加点も下げます。細分化コストは横セル数が2倍になるごとに加算し、拡大された入力で不要なセル分割を優遇しません。
10. 新規RGBAバッファへ決定済みセル色を直接描画しPNG encode。Preserveは元画像サイズへ1セル`P×P`画素、Logicalはグリッドサイズへ1セル1画素。アンチエイリアス、補間、半透明境界、サブピクセル座標は発生しません。

セル判定は元の配列を参照する二重バッファ方式で、処理順による色の伝播を避けます。近傍の列挙はヒープ確保なし。パレット間色差は先に計算し、各候補は逐次評価してメモリを再利用します。

縦横の細い直線がセル境界をまたぐ場合は、線方向の最大5セルで占有量を比較し、両側を線として選ぶことで生じる幅の揺れを抑えます。元画像で境界に接していること、3セル以上の直線の支持、十分に細いことを確認し、離れた平行線・太い線・曲がり角・交差部はこの補正から除きます。線の色が変わる場所では、背景との色差に対して十分近い色の細線を連続性の補助に使います。隣セルに元の色が存在する場合や、近い色が広い面を占める場合にはこの補助を適用しません。色を新設せず、元のセルにある色から選択します。

Autoでは線の中央色・連続性に加え、線幅も元画像と比較します。出力の同色領域を線の両側へ各64セルまで調べ、元画像上の幅へ換算するため、PreserveとLogicalで同じ評価になります。過度に太く／細くなった候補はShapeと境界整合度の加点を下げます。幅のある線には元の幅の25%（整数切り捨て）の境界誤差を許容し、1pxの線には余分な1pxを許容しません。探索上限で両端が不明な場合は確認できた幅による控えめな評価に留めます。

建物向けの保護ボーナス・領域係数・候補評価重みは`core/src/cell.rs`の`ShapeBalance` / `BUILDING`に分離しています。building / character / iconを選ぶプリセットUIは未実装です。新規依存は追加していません。

既存Pixel Art Fixer / Pixel Snapperのコードはコピーしていません。正規グリッドへの再構成とSurface Strict / Edge Strict / Shape Strictを独自実装しています。Autoと外形判定は軽量な近似で、物体の意味理解や元グリッドの厳密復元ではありません。量子化や任意のMedianで既に消えた色・細部は後段では復元できません。1セル内に複数の形がある場合は1セル1色の制約が優先されます。非整数拡大、オフセット、密な植物、意図的な色テクスチャはManual GridやShape強・Median Offでも比較してください。JPEGのEXIF回転・ICC色管理・アニメーションの再生はMVP対象外です。

## Rust CLI

Flutterなしで実行できます。入力／出力パスは日本語・空白に対応。既存ファイルは上書きしません。

```powershell
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- samples/house-pseudo.png output.png --grid auto --output preserve --colors 24 --smoothing 3 --edge 2 --shape 2 --report report.json
# 手動Grid（1254×1254入力なら3pxセル）
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- input.png strict.png --grid 418 --output preserve --colors 16 --median
# 従来の論理ピクセル原寸出力
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- input.png logical.png --grid 64 --output logical
```

`--output preserve|logical`は省略時`preserve`です。FFIのOptions JSONも`"output_mode":"preserve"` / `"logical"`を受け付け、旧JSONのように未指定ならPreserveになります。C ABIの関数・所有権は変更していません。

`--shape 1|2|3` / JSONの`shape_protection`は省略時2（中）です。既存JSONはそのまま受け付けます。Shape弱も保護を有効にした弱いボーナスで、旧アルゴリズムへ戻すモードではありません。

Reportの`classes`は **[FLAT, EDGE, DETAIL, UNKNOWN, LINE]** の5要素です。先頭4要素の位置を維持して末尾にLINEを追加しました。トップレベルに`silhouette_count`（外形候補セル数）、`preserved_detail_count`（保護色を残した小形状セル数。物体数ではない）、`line_continuity_score`、`shape_score`を追加。各候補の`metrics`にはさらに`silhouette_retention`、`line_continuity_retention`、`line_width_retention`、`small_detail_retention`、`corner_edge_consistency`を出します。保持指標は0〜1で、対象がない項目は1です。`shape_score`では線の連続性に線幅保持率を掛けて評価します。画質の保証値ではありません。PNGは決定論的ですが、Reportの`processing_ms`は実測時間なので変動します。

Reportは既存の`source_width` / `source_height` / `grid`等を保持し、`output_mode`、`output_width`、`output_height`、`cell_pitch`を追加します。`cell_pitch`は**出力上の正方形セルの一辺（px）**で、Logicalは常に1です。例: 1254×1254・Grid 418×418なら、Preserveは`output_width=1254, output_height=1254, cell_pitch=3`、Logicalは`output_width=418, output_height=418, cell_pitch=1`です。

`core/include/pixelstrict.h`が外部向けC ABIです。呼出元が`ps_alloc`した入力は`ps_dealloc`、返された結果は`ps_result_free`で1回解放します。JSONとPNGのポインタは結果ハンドルの解放まで有効です。処理エラーはJSONの`error`として返し、panicはFFI境界で捕捉します。Flutter側は結果をコピーしてからnativeメモリを解放し、`Isolate.run`で呼び出します。

## テスト・比較

```powershell
cargo test --locked --manifest-path core/Cargo.toml
cargo clippy --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings
cargo run --release --locked --manifest-path core/Cargo.toml --example samples -- samples
cargo run --release --locked --manifest-path core/Cargo.toml --example shape_samples -- samples/shape
python scripts/compare_samples.py  # Pillow 13以降。アプリ実行には不要
cd app
flutter analyze --no-pub
flutter test --no-pub
flutter test integration_test/conversion_test.dart -d windows --no-pub --dart-define=SAMPLE_PATH=C:/absolute/path/samples/house-pseudo.png
```

合成サンプルは実際のAI生成画像ではなく、正解グリッドが既知の画像に色ノイズ・境界混色を加えた制御実験です。`samples/comparison.png`は参照／入力／Auto／平滑化弱／強の比較、`samples/comparison.json`は色数・RGB誤差・処理時間です。時間はマシンと実行条件で変動します。実際のAI画像に対する広範な品質保証、異なるCPU間の浮動小数点演算を含むバイト一致は未検証です。

形の保護の再現用fixtureは[`samples/shape`](samples/shape/README.md)にあります。旧版との比較では、同じ解像度・強いsmoothingで消えていた低コントラストの窓枠・縁と小形状を保持し、単独ノイズの除去を維持しています。

ネイティブアプリのためWeb用PSI／Lighthouse／Chrome DevTools Issues監査の対象URLはありません。代わりにRust回帰テスト、Dart静的解析、画面サイズ別widgetテスト、Windows FFI統合テストと実画面確認を実施します。

## 他OS

| OS | ビルド | 前提 |
| --- | --- | --- |
| Windows x64 / ARM64 | `flutter build windows --release` | Windows、Visual Studio C++、対応Rust target |
| macOS Intel / Apple Silicon | `flutter build macos --release` | macOS、Xcode、CocoaPods、対応Rust targets |
| iOS device / simulator | `flutter build ios --release --no-codesign` / `flutter build ios --simulator` | macOS、Xcode、iOS 15+、署名は配布時に設定 |
| Android ARM / ARM64 / x64 | `flutter build apk --release` | Android SDK、NDK 27+、JDK、対応Rust targets |

画像選択・保存はfile_picker、D&Dはdesktop_dropを使用。macOS sandboxのuser-selected read/write entitlementを設定済みです。各OSのrunnerとnative asset hookを用意しており、FFI/core/UIは共通です。Android release署名はscaffoldのdebugキーのままで、ストア配布用ではありません。

Windows x64のReleaseビルド・UI操作・FFI統合テスト、Android ARM64のdebug APKビルドを確認済みです。macOS / iOS ARM64向けRustコアは`cargo check`に成功していますが、Apple向けアプリのリンク・署名・実機検証にはmacOS / Xcodeが必要です。Android端末でのファイル選択・保存・FFI動作は未検証です。Windows「送る」、macOS共有、iOS Share Extension / Shortcut、Android Share Intentは将来対応として未実装です。

Androidビルドではdesktop_drop 0.8.4の旧Kotlin Gradle Pluginに対する将来互換性警告と、ローカルSDKのXMLバージョン差の警告が出ます。ビルドは成功しています。前者は依存側の移行、後者はSDKツール更新が必要です。OSやSDKの設定変更は行っていません。Rust build hookはAndroid API 35固定を上書きし、Flutterから指定される最低対応API（現在24）に合わせます。

## アイコン

アプリアイコンは`app/assets/branding/pixelstrict.svg`の整数グリッドのPを使用します。Windowsの実行ファイル／ウィンドウ、macOS / iOSのAppIcon、Androidの通常／adaptive／monochromeアイコン、Flutter内ロゴへ設定済みです。画像の再出力は`python scripts/generate_icons.py`（開発時のみPillowが必要）。通常のビルド・アプリ実行にはPythonは不要です。

## 使用する主な依存

- Rust: image（PNG / JPEG / WebPのみ）、serde、serde_json。
- Flutter実行時: file_picker、desktop_drop。FFIはDart標準`dart:ffi`。
- ビルド時: hooks、code_assets、native_toolchain_rust。各依存のライセンスはパッケージに従います。
- [Flutter Native FFI / build hooks](https://docs.flutter.dev/platform-integration/bind-native-code)
- [native_toolchain_rust](https://pub.dev/packages/native_toolchain_rust)
- [file_picker](https://pub.dev/packages/file_picker)
