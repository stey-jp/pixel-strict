# 比較用サンプル

`house-reference.png`は32×32の手続き的な参照画像です。`house-pseudo.png`は6倍拡大に色ノイズと境界の混色を加えた192×192画像です。外部画像・画像生成APIは使用していません。

- `house-auto.png`: Grid / ColorsはAuto、smoothing/edgeは中。
- `house-surface-weak.png`: Grid 32、Colors 32、smoothing弱。
- `house-surface-strong.png`: Grid 32、Colors 32、smoothing強。
- 各JSON: 設定・分類件数・候補評価・処理時間。
- `comparison.png`: Nearest Neighborで拡大した横並び比較。
- `comparison.json`: 参照とのRGB絶対誤差、色数、処理時間。

比較用の生成スクリプトはLogical Resolutionを明示し、従来の32×32参照画像と同じ寸法を保ちます。アプリ・CLIのデフォルトはPreserve Resolutionです。既存のPNG・JSONは導入前の比較記録として保持しています。

```powershell
cargo run --release --locked --manifest-path core/Cargo.toml --example samples -- samples
python scripts/compare_samples.py
```

合成画像での制御実験です。実際のAI生成画像での画質評価を代替するものではありません。透過、細線、斜線、窓枠、孤立ノイズ、非整除サイズ、Median、PNG/JPEG/WebP入力はRust回帰テストでも検証します。
