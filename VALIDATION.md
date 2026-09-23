# 検証結果

## v0.1.8+9 — 境界の位置を保った陰影の選択（2026-09-23）

位置を固定した境界セルは明度補助の対象外だったため、同じ面に属する複数の色のうち、細線への加点が強い色を選ぶ問題が残っていました。境界の許容色マスクを変えず、採用する側の面の元明度を記録し、候補色選択へ加えました。反対側の面を平均に混ぜず、セル内の元画像を使います。追加状態は1セル4 bytes（最大4 MiB）。依存、パレット、処理の流れ、C ABI・Options・Report、UI配置、整数単色矩形描画は維持しています。

- 不透明・小形状保護なし・候補2色以上・候補間OKLab二乗距離0.004以内を条件にします。さらに明度幅10/255以上、最も近い色と次の色の誤差差4/255以上を要求。中間色付近や小さな差は従来の判断を維持します。1色に固定済みのセルは追加の明度計算を省略します。
- `shaded-stroke`の境界2行を含む全区間へ明度テストを拡張。v0.1.7+8で明度がずれた境界4セルが0セルになりました。`interrupted-stroke`は横棒の上下・4列すべてを検証し、対象48セルの明度ずれが**29→0セル**。両方とも縦横・Preserve / Logicalで確認し、線幅・端点・陰影の位置・横棒・小形状・切れ目を保持しています。
- `boundary-midpoint`を追加。暗い面に接する明るい面で、候補色の中間明度にノイズを加えても交互の色ムラを作らないことを検証。途中の陰影、縦横、両出力モード、全セル単色、PNG決定論性も確認しています。補正条件を絞る前に発生した新たな色ムラを、このテストで防ぎます。
- 実画像`original.png`をv0.1.7+8と同条件（Auto / Colors 32 / Smoothing 2 / Edge 2 / Shape 2 / Median Off）で比較。**Grid 418×418・P=3、1254×1254・32色**は不変。169セル（1,521画素）の色が変化し、alphaは不変です。変更セルと元の3×3セル平均との明度MAEは**25.87→19.29**（125セル改善、44セル悪化）、RGB MAEは**24.99→19.10**（123セル改善、46セル悪化）。境界では片側の面を選ぶため、背景を含むセル全体との誤差は参考値であり、完全な形状保持の証明ではありません。
- 窓枠の区間（x=534..536、y=420..494、25セル）では、元セル平均との明度MAEが**15.41→13.26**。一方、隣接セルの明度差が8/255を超える箇所は**5→7**となり、細かな色の切り替わりは解消していません。x=525 / 528 / 531の同区間は不変です。元画像と派生比較はローカルのみに保存しています。
- 既存houseのAuto / Surface弱 / Surface強はPNGバイト単位で不変。RGB MAE **2.819 / 2.654 / 2.663**を維持。既存Shape fixtureでは`straight-facade`、`blended-frame`、`shaded-stroke`、`interrupted-stroke`のManualと`shaded-stroke`のAutoの色が変化し、選択Gridは維持しました。他のPNGは不変。回帰用出力・JSONを更新しています。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **42件成功**（1件追加、既存2件の検証範囲を拡張）。既存の外形、斜線、平行線、小形状、単独ノイズ、透明境界、Auto、非整除Logical、Preserveも成功。
- `cargo fmt --manifest-path core/Cargo.toml --check`、`cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`flutter analyze --no-pub`: **成功、指摘なし**。
- `flutter test --no-pub`、Windows FFI統合テスト**2件**、`flutter build windows --release --no-pub`: **成功**。実画像の画面で固定ボタンと時間の配置を確認し、同期preview、保存バイト列、設定変更時の無効化を維持。画面・exe・同梱pubspecの版は**0.1.8+9**です。
- 配布用DLLで実画像・7種のfixtureが検証済みPNGと全画素一致。実画像のPreserveはLogicalの3×3単色矩形と全画素一致。旧／新をwarm-up後、順序を交互に各5回計測した中央値は**367.3ms→366.5ms**で、今回のローカル計測ではほぼ同等でした。

候補色が曖昧な境界や境界判定が途切れる区間には色ムラが残ります。次の改善候補は、境界位置と本来の陰影を守りながら、隣接する行の色の切り替わりを評価することです。Android / macOS / iOSは今回再ビルド・実機検証していません。ネイティブアプリのためPSI / Lighthouse / CDPの対象URLなし。以下は過去版の記録です。

## v0.1.7+8 — 端点・交差部近くの線の明度を安定化（2026-09-23）

前回の明度補助は前後2セルずつの支持を必須にしていたため、端点や陰影の切り替わり近くでは補正できませんでした。Rust coreの`coherent_tone`の参照範囲だけを調整し、同じ陰影が続く側から合計5セルを集めます。方向ごとに最大4セル、途中で明度・色のまとまり・線方向が変わればその側の探索を終了。必要な支持数と3色以上の競合条件は維持し、短い線や切れ目を飛び越えて補正しません。追加バッファ・依存なし。C ABI・Options・Report・処理の流れ・UI配置・単色矩形描画を維持しています。

- `shaded-stroke`の検証範囲を線の両端まで拡張。旧v0.1.6+7で最初の端点セルが失敗することを確認し、修正後は成功。明暗の切り替わりでは、境界位置を保護する2行を除いてその直前・直後まで元の明度に近づけます。境界2行は従来の位置制限を優先し、明暗の変化位置を保ちます。
- `interrupted-stroke`を追加。横棒付近の色ムラ、横棒の連続性、2セルの色付き小形状、6pxの切れ目、線幅と端点を検証。2セルごとの明暗変化と透明な切れ目を持つ別ケースも追加し、縦横・Preserve / Logicalで本来の変化を残すことを確認しました。
- 実画像`original.png`をv0.1.6+7と同条件（Auto / Colors 32 / Smoothing 2 / Edge 2 / Shape 2 / Median Off）で比較。**Grid 418×418・P=3、1254×1254・32色**は不変。112セル（1,008画素）の色が変わり、元の3×3セル平均との明度誤差は111セルで減少、1セルで増加。変更セルの明度MAEは**13.17→2.51**、RGB MAEは**13.05→3.82**（109セルで減少、3セルで増加）でした。画像全体の品質を保証する指標ではありません。
- 前回と同じ窓枠の代表区間（x=531..533、y=420..494、25セル）では、隣接セルの明度差が8/255を超える箇所が**3→2**、元のセル平均との明度MAEが**4.29→3.18**。隣接するx=525 / 528 / 534の同区間の色列は不変。元画像・派生比較画像はローカルのみに保存しています。
- 既存houseのAuto PNGは不変。Surface弱は5セル、強は3セルが変化し、正解参照とのRGB MAEは弱 **2.670→2.654**、強 **2.678→2.663**。1セルではRGB誤差が3.67→5.00に増加する小さな差があり、全差分を確認したうえでPNG goldenとReport・比較資料を更新。Grid 32を維持し、色数は11 / 19 / 16。既存Shape fixtureのPNGは`blended-frame`と`shaded-stroke`のManualのみ変化し、AutoのPNGはすべて不変です。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **41件成功**（追加2件＋既存の端点検証を拡張）。外形、斜線、平行線、単独ノイズ、小形状、Auto、非整除Logical、Preserve、決定論性も成功。
- `cargo fmt --manifest-path core/Cargo.toml --check`、`cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`flutter analyze --no-pub`: **成功、指摘なし**。
- `flutter test --no-pub`、Windows FFI統合テスト**2件**、`flutter build windows --release --no-pub`: **成功**。実画像の変換画面を確認し、固定ボタン・時間表示・同期preview・保存バイト列・両出力モード・設定変更時の無効化を維持。画面、exe、同梱pubspecの版は**0.1.7+8**です。
- 配布用DLLで実画像と6種のfixtureが検証済みPNGと全画素一致。実画像のPreserveはLogicalを3×3単色矩形にした画像と全画素一致しました。旧／新をwarm-up後に順序を交互に各5回計測した中央値は**414.8ms→411.0ms**。各回のばらつきが大きく、速度改善を示す結果とは扱いません。

2色だけが競合する線、境界位置保護のあるセル、支持が5セルに満たない短い線には色ムラが残ります。次の改善候補は、形状を固定したまま境界セル内の陰影を評価することです。Android / macOS / iOSは今回再ビルド・実機検証していません。ネイティブアプリのためPSI / Lighthouse / CDPの対象URLなし。以下は過去版の記録です。

## v0.1.6+7 — 直線内部の明暗の乱れを抑制（2026-09-23）

境界の位置を保護しても、窓枠内部の細い陰影にLINE加点が入り、行ごとに暗い色と明るい色を選ぶ問題が残っていました。元画像の明るさが安定した縦横の直線では、セルの平均明度との誤差を色選択へ追加しました。preprocess / quantize / grid / analyze / decide / evaluate、C ABI・Options・Report、整数セル描画は維持。新規依存なし。右上の版表示・exe製品バージョンは**0.1.6+7**です。

- 近い3色以上でセルの88%以上を占め、同方向の線が前後2セルずつ続く場合のみ候補にします。5セルの元明度差は6/255以内、3セル以上で色の競合が必要。境界制限・小形状保護のあるセルと透明画素を含むセルは除外し、既存の境界位置と細線の配置制限を優先します。元セルの色だけを使い、線幅・描画方式は変えません。
- `shaded-stroke`を追加。安定部分の明度、元の明暗変化、線幅と端点、縦横、Preserve / Logical、全セルの単色矩形と決定論性を確認。横棒と2セルの色付き小形状の保持も別テストで検証しています。
- ユーザー提供の`original.png`をv0.1.5+6と同条件（Auto / Colors 32 / Smoothing 2 / Edge 2 / Shape 2 / Median Off）で比較。両方とも**Grid 418×418・P=3、出力1254×1254・32色**です。窓枠の代表区間（x=531..533、y=420..494、25セル）で、隣接セルの明度差が8/255を超える箇所が**7→3**、元の3×3セル平均との明度MAEが**11.86→4.29**。隣接するx=525 / 528 / 534の同区間の色列は不変です。全体の変更は4,410画素。これは代表区間の局所評価であり、全ての縦線が直ったことを示す指標ではありません。元画像と派生画像はローカルのみに保存しています。
- 既存houseはAutoの出力PNGが不変。Surface弱は5セル、強は4セルの色が変わり、全差分が正解参照へ近づくことを確認しました。RGB MAEはAuto **2.819（不変）**、弱 **2.725→2.670**、強 **2.725→2.678**。Grid 32は維持、色数は11 / 20 / 17。PNG goldenとReport・比較資料を更新しました。既存Shape fixtureでは`blended-frame`のManualのみPNGが変化し、ガラス側へのはみ出し0行・全32行の窓枠保持は継続しています。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **39件成功**（今回2件追加）。斜線、平行線、交差部、外形、小形状、単独ノイズ、Auto、非整除Logical、Preserveも成功。
- `cargo fmt --manifest-path core/Cargo.toml --check`、`cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`flutter analyze --no-pub`: **成功、指摘なし**。
- `flutter test --no-pub`、Windows FFI統合テスト**2件**: **成功**。固定ボタン・版表示・時間表示・両出力モード・同期preview・保存バイト列・設定変更時の無効化を確認。実画像の変換画面も取得して表示を確認しました。
- `flutter build windows --release --no-pub`: **成功**。配布用DLLで実画像と5種のfixtureが検証済みPNGと全画素一致。実画像のPreserveがLogicalの3×3単色矩形と全画素一致し、exeと同梱pubspecの版も一致しました。

追加状態はセル平均明度4 bytes（上限1,048,576セルで4 MiB）。配布用の旧／新DLLで実画像をwarm-up後、順序を交互に各5回計測した中央値は**366.4ms→375.3ms（約2.4%増）**。decode〜PNG encodeのローカル計測で負荷により変動します。

端点、明暗が切り替わる場所、2色だけが競合する線には色ムラが残ります。パレットに適切な中間色がなければ再現できず、1セルより細い形も制約があります。次の改善候補は、端点や交差部で意図的な陰影を守りながら連続性を評価することです。Android / macOS / iOSは今回再ビルド・実機検証していません。ネイティブアプリのためPSI / Lighthouse / CDPの対象URLなし。以下は過去版の記録です。

## v0.1.5+6 — 減色前の境界位置を参照（2026-09-23）

ぼけを含む窓枠では、減色後の中間色が細線として扱われ、同じ直線の途中で面へのはみ出しや欠けが残っていました。従来の色による境界確認で判定できない場合に、減色前の明度を使う補助を追加。元の処理順、C ABI・Options・Report形式、整数セル描画、依存関係、前回のUI整理は維持しています。

- 不透明な両面の明度差が24/255を超え、局所的な変化が両面の差の25%以上ある境界だけを対象とします。最大5セル・各3走査線、最低9点・80%以上の支持が必要。ぼけによる1pxの位置差は、同じ側のセルへ丸められる場合のみ許容します。緩やかな面のグラデーション、透明境界の明度推測には適用しません。
- 補助が候補色を絞らないセルは保護対象に加えず、通常の面統合を維持。建物の外側に色ムラを固定してしまう問題を防ぎ、平らな背景の回帰テストを追加しました。
- `blended-frame`を追加。旧v0.1.4+5では窓枠の中間色がガラス側へ出る行が**32行中28行→0行**。隣の窓枠は全32行で保持。16 / 24 / 32色、縦横、Preserve / Logical、全セル単色、決定論性を検証しています。本来の階段状の変化と6pxの切れ目を維持する別テストも追加。
- ユーザー提供の`original.png`は同じAuto / Colors 32 / Smoothing 2 / Edge 2 / Shape 2 / Median Offで比較。採用グリッドは従来どおり**418×418・P=3、1254×1254出力**。窓枠と柱の周囲を拡大比較し、境界の中間色によるはみ出しが減ることを確認。元画像と比較画像はローカルのみに保存しています。
- 既存houseは正解参照とのRGB MAEがAuto **3.113→2.819**、Surface弱 **3.255→2.725**、強 **3.258→2.725**。Grid 32は維持し、出力色数は11 / 21 / 18。主に建物左右の境界とドア周囲の中間色が整理されました。全差分を確認してPNG golden、Report、比較資料を更新。従来6種のShape fixtureのPNGは不変です。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **37件成功**。今回3件追加。既存の斜線、平行線、交差部、外形、小形状、単独ノイズ、1254pxのPreserve、非整除Logical、Autoも成功。
- `cargo fmt --manifest-path core/Cargo.toml --check`、`cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`flutter analyze --no-pub`: **成功、指摘なし**。
- `flutter test --no-pub`、Windows FFI統合テスト**2件**: **成功**。固定ボタン、版表示、処理時間の配置、変換・保存バイト列・両出力モード・同期preview・設定変更後の無効化を確認。実画像を読み込んだ実描画もPNGで確認。
- `flutter build windows --release --no-pub`: **成功**。同梱DLLで新fixtureと既存4fixture、実画像の検証済み出力との全画素一致を確認。実画像のPreserveはLogicalの3×3単色矩形と一致。exeのFileVersion / ProductVersionと画面の表示は**0.1.5+6**です。

明度キャッシュは入力1画素につき1 byte（1254×1254で約1.5 MiB、最大16 MiB）。RGBA入力を追加で保持しません。実画像のAuto変換を旧／新同梱DLLでwarm-up後、交互に各5回計測した中央値は**356.0ms→381.8ms（約7.3%増）**でした。decode〜PNG encodeのローカル計測で、負荷により変動します。

明度差が小さい縁、細線内部の色ムラ、密な植物、量子化で同色になった細部には限界があります。1セルより細い形は完全には再現できません。次の候補は、同じ線の内側の色変化と意図的な陰影を区別する評価の追加です。Android / macOS / iOSは今回再ビルド・実機検証していません。以下は過去版の記録です。

## v0.1.4+5 — 柱の直線境界とUI整理（2026-09-23）

前回の細線補正だけでは、広い柱の側面を行ごとに異なる色・位置で選ぶ問題が残っていました。元画像で確認できる縦横の直線境界をanalyzeで記録し、decideで同じ側の面の色へ候補を制限しました。最大5セル内の3走査線ずつ、最低9点・80%以上の位置一致を要求。隣接セルの75%以上が同じ面であることを確認し、近接した別々の細線をまとめないようにしています。追加状態はセル当たり4 bytes。C ABI・Options・Report形式・描画・依存関係は変更なし。

- ユーザー提供の`original.png`（1254×1254）を、旧`3524a83`と同じAuto / Colors 32 / Smoothing 2 / Edge 2 / Shape 2 / Median Offで比較。両方ともGrid 418×418・P=3。柱と窓枠のギザギザが減ることを拡大比較しました。代表区間（入力x=405..424、y=525..584）の柱の境界は元画像でx=412。P=3の出力ではx=411を期待し、20セル行のうちx=414へずれる行が**12→0**になりました。画像全体の完全復元を示す指標ではありません。元画像・派生比較画像はローカルのみに保存しています。
- 合成`straight-facade`を追加。色ノイズと横帯があっても、縦横とも境界を一定位置に保持。Preserve / Logical、全画素の単色矩形、同一PNGの決定論性を確認しました。既存houseのPNG goldenと従来5種のShape fixtureの出力PNGは不変。粗い候補の指標が変わるbuilding-likeのJSONと新fixtureを更新しました。
- UI: 右上は**v0.1.4+5**。同梱pubspecから取得し、Windows exeのFileVersion / ProductVersionとも一致。キャッチフレーズ、冗長なバッジ、フッターステータスバーを削除。変換・保存はスクロール外に固定し、処理時間はGridの直下に1回だけ表示します。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **34件成功**。斜線・細線・近接した平行線・交差部・外形・小形状・単独ノイズ・両出力モードの既存回帰も成功。
- `cargo fmt --manifest-path core/Cargo.toml --check`、`cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`flutter analyze --no-pub`: **成功、指摘なし**。
- `flutter test --no-pub`: **成功**。390×844 / 1280×800 / 1280×600で版表示、文言削除、操作の無効化、設定をスクロールしてもボタン位置が変わらないこととoverflowなしを検証。
- Windows FFI統合テスト: **2件成功**。既存の変換・保存バイト列・同期preview・設定変更後の無効化に加え、時間表示の順序／重複なし、版表示、実画像の読み込み・変換・固定ボタンを検証。Flutterの実描画をPNGに取得して配置を目視確認。
- `flutter build windows --release --no-pub`: **成功**。Release同梱DLLが新fixture・既存3fixtureおよび実画像の検証済み出力と一致。実画像でもPreserve出力がLogical出力の3×3単色矩形と全画素一致しました。

同梱DLLで実画像のAuto変換をwarm-up後、旧／新を交互に各5回計測した中央値は**345.8ms→353.9ms（約2.3%増）**。これはローカルのdecode〜PNG encode計測で、マシン負荷による変動があります。

低コントラストの縁、境界付近のハイライト、交差部や密な植物にはまだ揺れが残ります。3pxセルより細い形の再現には制約があり、緩い斜線や入力自体の位置の揺れをすべて補正するものではありません。次の改善候補は、元画像の明度境界を参照した量子化前の位置検出と、実画像での境界位置保持率の評価です。Android / macOS / iOSは今回再ビルド・実機検証していません。以下は過去版の検証履歴です。

## 縦線の幅・位置・連続性の改善（2026-09-23）

セル境界にかかった縦線の両側へLINE加点が入り、行ごとに幅が1〜2セルへ変わる問題と、近い色への変化で同色の連続性が失われる問題を合成fixtureで再現しました。Rust coreのanalyze / decide / evaluate内で対応し、UI・Options・Report形式・C ABI・描画処理・依存関係は変更していません。

- 元画像で境界に接する細線について、線方向の最大5セルの占有量から配置先を決めます。3セル以上の直線の支持と2か所以上の境界接続を要求し、離れた平行線・太い柱・交差部の統合を避けます。縦横に同じ規則を使い、斜線の位置をそろえる処理は追加していません。
- 背景との色差の1/8以内、かつOKLab二乗距離0.02以内の色を線の支持候補にします。隣セルに同じ色がなく、同方向の細線がある場合だけ補助し、広い面の色は支持に使いません。出力色は各セルに実在する色から選びます。半セル幅の直線は高コントラストの場合のみLINE判定を補強し、端点も保持します。
- `vertical-boundary`（96×96、Grid 24、P=4、Colors 32、Smoothing 3）では、旧`2b2a967`の余分な線セル24個が0個へ。3本×20セルを一定の位置・幅で保持します。`vertical-shades`では旧版で消えた3本×20セルを端点まで保持。入力線は2pxなので、P=4での最小出力幅は4pxです。
- 既存houseのAuto / Surface弱 / Surface強のPNGは**直前版とバイト単位で一致**。Autoは32×32・11色、Manualは弱24色／強20色、RGB MAEは3.113／3.255／3.258を維持。既存3種のShape fixtureのPNGも不変。JSON reportと比較資料は再生成しました。
- `cargo test --release --locked --manifest-path core/Cargo.toml`: **33件成功**。追加2件で幅・位置・色変化・端点・横方向への回転・離れた平行線・太い柱・交差部を検証。Preserveの全セルが単色矩形、Logicalの対応、同一PNGの決定論性も確認。既存の斜線・外形・小形状・単独ノイズ・197/1254px非整数拡大のAuto回帰も成功。
- `cargo clippy --release --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings`、`cargo fmt --manifest-path core/Cargo.toml --check`、`flutter analyze --no-pub`: **成功、指摘なし**。
- Windows FFI統合テスト: **成功**。変換、両出力モード、Report読取、同期preview、設定変更後の結果無効化、再変換と保存を確認。
- `flutter build windows --release --no-pub`: **成功**。同梱DLLを直接C ABIで呼び、縦線2fixtureのPreserve / Logical両方で20行にわたる一定の幅・位置・連続性を確認。Preserveは生成済みfixtureの全画素とも一致。成果物は`app/build/windows/x64/runner/Release/`です。

同じhouse画像を旧`2b2a967`／新Release同梱DLLへ渡し、warm-up後に実行順を交互にして各5回計測した中央値（decode〜PNG encode）:

| 入力・設定 | 直前版 | 縦線改善後 |
| --- | ---: | ---: |
| 192×192・Auto Logical | 39.2ms | 45.7ms |
| 1254×1254・Auto Logical | 150.2ms | 160.7ms |
| 1254×1254・Preserve Grid 418・Colors 24・Smoothing 3 | 213.1ms | 230.7ms |

1254版はNearest Neighborで拡大した入力です。この範囲で約7〜17%の処理時間増があり、元のShape protectionによるコスト増も引き続き存在します。サンプルJSONの時間は並行ビルド中の単発値であり、この比較計測とは別です。

追加状態はセルごとの境界色ビット集合4個（16 bytes）。色の候補集合は最大32色、直線の比較は最大5セルで、反復的な画像平滑化や補間は行いません。AutoがP=1を選ぶ場合は入力の位置の揺れも保持します。低コントラストで複数の色が混ざる線、実際の建物画像、緩い傾きや密な植物での誤判定は引き続き検証が必要です。Android / macOS / iOSの再ビルド・実機検証は未実施。

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
