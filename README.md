# PixelStrict

AI生成の疑似ピクセルアートを、整数グリッド・1セル1色のPNGへ再構成するローカルアプリです。Flutter UI / Rust core / C ABI FFI。OpenAI API・画像生成・OpenCVは使用しません。

Preserve対応のWindows Releaseは`flutter build windows --release`で生成する`app/build/windows/x64/runner/Release/`です。初回MVPのビルド: `output/PixelStrict-windows-x64.zip`、`output/PixelStrict-android-arm64-debug.apk`（今回未更新）。[検証結果](VALIDATION.md)と[比較画像](samples/comparison.png)も参照してください。Windows配布時はexe単体ではなくDLL・dataを含むReleaseフォルダー全体が必要です。

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
5. Surface smoothing / Edge protectionを弱・中・強から選択。MedianはOff / 半径1px。
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
6. セル内占有率、近似色の合計占有率、色ごとの空間共分散、周囲8セルのコントラストからFLAT / EDGE / DETAIL / UNKNOWNに分類。
7. `占有率 + 周囲の連続性 + 方向に沿う連続性 − 孤立ペナルティ − 不要な色差`で最終色を決定。方向は水平・垂直・2斜線の正負8方向。連続線は少数派色も候補に残します。DETAILでは面統合を弱め、UNKNOWNは保守的に判定。
8. FLATを起点に同一面の近似色を再統合。固定seedとの色差で制限し、段階的に異なる面へ色が伝播する連鎖を防ぎます。輪郭セルは同じ面の色が88%以上を占める場合のみ統合対象。
9. 面均一性、エッジ連続性、孤立画素数、量子化後の入力との差、境界整合度と過剰な細分化への小さなペナルティを比較し候補を採用。全候補の内訳はCLIのJSON reportに出力。
10. 新規RGBAバッファへ決定済みセル色を直接描画しPNG encode。Preserveは元画像サイズへ1セル`P×P`画素、Logicalはグリッドサイズへ1セル1画素。アンチエイリアス、補間、半透明境界、サブピクセル座標は発生しません。

セル判定は元の配列を参照する二重バッファ方式で、処理順による色の伝播を避けます。近傍の列挙はヒープ確保なし。パレット間色差は先に計算し、各候補は逐次評価してメモリを再利用します。

既存Pixel Art Fixer / Pixel Snapperのコードはコピーしていません。元グリッドを厳密復元する方式ではなく、正規グリッドへの再構成とSurface Strict / Edge Strictを独自実装しています。Autoはヒューリスティックであり、元解像度の正解を保証しません。非整数拡大、オフセット、意図的な色テクスチャ、1セル内の極小ディテールは手動Gridと弱い平滑化で比較してください。JPEGのEXIF回転・ICC色管理・アニメーションの再生はMVP対象外です。

## Rust CLI

Flutterなしで実行できます。入力／出力パスは日本語・空白に対応。既存ファイルは上書きしません。

```powershell
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- samples/house-pseudo.png output.png --grid auto --output preserve --colors 24 --smoothing 3 --edge 2 --report report.json
# 手動Grid（1254×1254入力なら3pxセル）
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- input.png strict.png --grid 418 --output preserve --colors 16 --median
# 従来の論理ピクセル原寸出力
cargo run --release --manifest-path core/Cargo.toml --bin pixelstrict -- input.png logical.png --grid 64 --output logical
```

`--output preserve|logical`は省略時`preserve`です。FFIのOptions JSONも`"output_mode":"preserve"` / `"logical"`を受け付け、旧JSONのように未指定ならPreserveになります。C ABIの関数・所有権は変更していません。

Reportは既存の`source_width` / `source_height` / `grid`等を保持し、`output_mode`、`output_width`、`output_height`、`cell_pitch`を追加します。`cell_pitch`は**出力上の正方形セルの一辺（px）**で、Logicalは常に1です。例: 1254×1254・Grid 418×418なら、Preserveは`output_width=1254, output_height=1254, cell_pitch=3`、Logicalは`output_width=418, output_height=418, cell_pitch=1`です。

`core/include/pixelstrict.h`が外部向けC ABIです。呼出元が`ps_alloc`した入力は`ps_dealloc`、返された結果は`ps_result_free`で1回解放します。JSONとPNGのポインタは結果ハンドルの解放まで有効です。処理エラーはJSONの`error`として返し、panicはFFI境界で捕捉します。Flutter側は結果をコピーしてからnativeメモリを解放し、`Isolate.run`で呼び出します。

## テスト・比較

```powershell
cargo test --locked --manifest-path core/Cargo.toml
cargo clippy --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings
cargo run --release --locked --manifest-path core/Cargo.toml --example samples -- samples
python scripts/compare_samples.py  # Pillow 13以降。アプリ実行には不要
cd app
flutter analyze --no-pub
flutter test --no-pub
flutter test integration_test/conversion_test.dart -d windows --no-pub --dart-define=SAMPLE_PATH=C:/absolute/path/samples/house-pseudo.png
```

合成サンプルは実際のAI生成画像ではなく、正解グリッドが既知の画像に色ノイズ・境界混色を加えた制御実験です。`samples/comparison.png`は参照／入力／Auto／平滑化弱／強の比較、`samples/comparison.json`は色数・RGB誤差・処理時間です。時間はマシンと実行条件で変動します。実際のAI画像に対する広範な品質保証、異なるCPU間の浮動小数点演算を含むバイト一致は未検証です。

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
