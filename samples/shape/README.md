# Shape protection fixtures

実際のAI画像ではなく、形を保持できたかを判定するための合成fixtureです。`core/tests/common/shapes.rs`から再生成できます。

```powershell
cargo run --release --locked --manifest-path core/Cargo.toml --example shape_samples -- samples/shape
```

- `building-like`: 低コントラストの屋上の縁・柱・窓枠・角、植物、照明、単独ノイズ。
- `line-heavy`: 高コントラストの縦横線と斜線。Autoで線を失う粗い候補を選ばないことも検証。
- `flat-with-noise`: 同じ面の単独ノイズ5点と、近似色の2×3セルの小形状。
- `vertical-boundary`: セル境界をまたぎ位置が1px揺れる縦線3本。
- `vertical-shades`: 色が途中で変わる細い縦線3本。
- `straight-facade`: 左側の横帯と右側の色ノイズに挟まれた、x=31の直線境界。96×96・Manual Grid 32（P=3）。

`*-input.png`が入力、`*-manual.png`はPreserve・Colors 32・Smoothing 3・Edge 2・Shape 2、`*-auto.png`は同設定でGrid Autoです。対応JSONに候補評価を出力します。最初の3例は48×48・Manual Grid 48（P=1）。`vertical-*`は96×96・Manual Grid 24（P=4）。拡大時にも補間しないで比較してください。

`vertical-boundary`のManualは旧`2b2a967`で左右に余分なセルが生じましたが、直線の位置調整後は3本とも20セルにわたって一定の位置・1セル幅を保持。`vertical-shades`では消えていた3本を端点まで保持します。横線への90度回転、両出力モード、単色矩形と決定論性もテストします。P=4のため2pxの入力線も出力は4px幅です。AutoがP=1を選ぶ場合は元の揺れも保持し、任意の画像の線をすべて直線化する処理ではありません。

線幅評価の追加後、`line-heavy`のAutoは旧`672740c`のGrid 12（P=4）からGrid 48（P=1）へ変更。縦横の1px線と斜線の太さ・位置が入力と一致します。両解像度モードで1/2/4倍入力と上下反転を回帰テストし、拡大済みの線を不要に細くしないことも確認しています。各候補の`metrics.line_width_retention`で線幅保持率を確認できます。

旧版`9f081f1`の同じManual設定との比較:

| 確認対象 | 旧版 | Shape改善後 |
| --- | ---: | ---: |
| 建物の低コントラストの縁・窓枠（174セル） | 0保持 | 174保持 |
| 面内の小形状（6セル） | 0保持 | 6保持 |
| 単独面ノイズ（5点） | 5除去 | 5除去 |

比較はRGB値と位置の完全一致で集計。高コントラストの線fixtureは旧版でも保持され、新版でも継続します。`core/tests/shape.rs`でさらに透明な突起・少数派の線と端点・8方向の1〜2セル幅の線・2〜6セルの連結形状・Autoの元画像参照評価・両出力モード・決定論性を検証します。

`straight-facade`は、元の境界を3pxグリッド上のx=30へ一定に配置できることを検証します。右側を占める2/3の色を選び、横帯や面のノイズによって行ごとの位置を変えません。縦横、Preserve / Logical、全セルの単色矩形と決定論性を回帰テストしています。

fixtureでの結果は実画像全般に対する品質保証ではありません。Medianと量子化による前段の情報損失、1セル1色より細かい形の再現、密な植物や非整数グリッドは引き続き制限があります。
