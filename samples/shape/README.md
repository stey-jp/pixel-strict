# Shape protection fixtures

実際のAI画像ではなく、形を保持できたかを判定するための合成fixtureです。`core/tests/common/shapes.rs`から再生成できます。

```powershell
cargo run --release --locked --manifest-path core/Cargo.toml --example shape_samples -- samples/shape
```

- `building-like`: 低コントラストの屋上の縁・柱・窓枠・角、植物、照明、単独ノイズ。
- `line-heavy`: 高コントラストの縦横線と斜線。Autoで線を失う粗い候補を選ばないことも検証。
- `flat-with-noise`: 同じ面の単独ノイズ5点と、近似色の2×3セルの小形状。

`*-input.png`が入力、`*-manual.png`はGrid 48・Preserve・Colors 32・Smoothing 3・Edge 2・Shape 2、`*-auto.png`は同設定でGrid Autoです。対応JSONに候補評価を出力します。画像は48×48で、ManualはP=1。拡大時にも補間しないで比較してください。

線幅評価の追加後、`line-heavy`のAutoは旧`672740c`のGrid 12（P=4）からGrid 48（P=1）へ変更。縦横の1px線と斜線の太さ・位置が入力と一致します。両解像度モードで1/2/4倍入力と上下反転を回帰テストし、拡大済みの線を不要に細くしないことも確認しています。各候補の`metrics.line_width_retention`で線幅保持率を確認できます。

旧版`9f081f1`の同じManual設定との比較:

| 確認対象 | 旧版 | Shape改善後 |
| --- | ---: | ---: |
| 建物の低コントラストの縁・窓枠（174セル） | 0保持 | 174保持 |
| 面内の小形状（6セル） | 0保持 | 6保持 |
| 単独面ノイズ（5点） | 5除去 | 5除去 |

比較はRGB値と位置の完全一致で集計。高コントラストの線fixtureは旧版でも保持され、新版でも継続します。`core/tests/shape.rs`でさらに透明な突起・少数派の線と端点・8方向の1〜2セル幅の線・2〜6セルの連結形状・Autoの元画像参照評価・両出力モード・決定論性を検証します。

fixtureでの結果は実画像全般に対する品質保証ではありません。Medianと量子化による前段の情報損失、1セル1色より細かい形の再現、密な植物や非整数グリッドは引き続き制限があります。
