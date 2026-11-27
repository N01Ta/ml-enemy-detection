# Dataset notes

The detector was trained on **54,644 Apex Legends Firing Range frames**.

| Split | Images |
| --- | ---: |
| Train | 48,490 |
| Validation | 5,314 |
| Test | 840 |

| Class | ID | Boxes |
| --- | ---: | ---: |
| Enemy | 0 | 55,348 |
| Friendly | 1 | 956 |

- The original image archive is **4.1 GB** and is **not redistributed**.
- The original signed `apex-2.ndjson` manifest and its CDN URLs are **not
  included** in this repository.
- The raw dataset label is spelled `Freindly`; the runtime maps class `1` to
  the friendly color.

## Using your own dataset

`train_python/train.py` accepts a manifest via `DATASET`:

```powershell
$env:DATASET = "C:\path\to\your-dataset.ndjson"
cd train_python
python train.py
```

If `DATASET` is not set, the script looks for `../apex-2.ndjson` next to the
repository. That file is ignored by Git.

The expected format is an Ultralytics-compatible NDJSON export with
`task: detect`, image entries and box annotations. Class `0` must be the
enemy/opponent and class `1` the friendly/teammate if you want to reuse the
existing runtime colors.

## Other games

The pipeline is game-agnostic, but the weights were trained on Apex Legends.
For a different game, expect to:

1. Capture frames from that game.
2. Label enemy/teammate boxes.
3. Fine-tune `best.pt` or train from `yolo26n.pt`.
4. Re-export to `rust_run/enemy_detector.onnx`.

Zero-shot cross-game accuracy has not been benchmarked.

## Licensing

The dataset and gameplay footage belong to their respective owners. No raw
images, textures, models or audio are redistributed. The demo video and
screenshots are included for documentation only.
