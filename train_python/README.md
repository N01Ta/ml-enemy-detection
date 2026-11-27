# Training pipeline

Python pipeline for training a game character detector (trained here on Apex
Legends) and exporting it to ONNX for the Rust runtime.

## Requirements

- Windows 10/11
- Python 3.11 or 3.12 (recommended; CUDA wheels are not guaranteed on newer Python)
- NVIDIA GPU + CUDA 12.1 for GPU training (CPU works but is very slow)
- ~4.5 GB free disk space for the dataset and checkpoints

## 1. Install

```powershell
cd train_python
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121
pip install -r requirements.txt
```

`requirements.txt` installs Ultralytics, ONNX, Pillow and NumPy. PyTorch is
installed separately so the CUDA build matches your driver.

## 2. Check CUDA

```powershell
python check_gpu.py
```

Expected output includes the GPU name, VRAM and `CUDA available: True`.

## 3. Dataset

`train.py` accepts a dataset manifest through the `DATASET` environment
variable:

```powershell
$env:DATASET = "C:\path\to\your-dataset.ndjson"
python train.py
```

If `DATASET` is not set, it falls back to `../apex-2.ndjson` next to the
repository.

The original training set was **54,644 Apex Legends images**:

| Split | Images |
| --- | ---: |
| Train | 48,490 |
| Validation | 5,314 |
| Test | 840 |

The raw image archive is **4.1 GB** and is **not redistributed** in this
repository. The original signed manifest/CDN URLs are not included either.
Export your own manifest from the
[Ultralytics Platform](https://platform.ultralytics.com/) or another YOLO
dataset tool; see [`../dataset/README.md`](../dataset/README.md) for the split
and re-export notes.

Classes: `Enemy` (0) and `Friendly` (1). The raw dataset label is misspelled
`Freindly`; the runtime maps class 1 to the friendly color.

## 4. Train

```powershell
python train.py
```

The script trains `yolo26n.pt` for 30 epochs at 640×640, batch 32, on the
first CUDA device:

- `AMP` is disabled for cuDNN stability on Windows.
- `cudnn.deterministic` / `benchmark` are configured in `train.py`.
- Output: `runs/apex_yolo/weights/best.pt` and `last.pt` (only `best.pt` is
  committed; checkpoints are otherwise ignored).
- Metrics/curves: `runs/apex_yolo/results.csv`, `results.png`,
  `confusion_matrix.png`, `BoxPR_curve.png`.

Training on the reference RTX 4070 Ti took ~3.1 hours (`11,157 s`). Lower
`batch` to 16 if you run out of VRAM.

## 5. Export to ONNX

```powershell
python export_onnx.py
```

Exports FP16, fixed 640×640 ONNX and copies it to:

```text
../rust_run/enemy_detector.onnx
```

The runtime expects that filename in `rust_run/`. The export step also creates
`runs/apex_yolo/weights/best.onnx`; only the runtime copy is committed.

## Validation metrics

From the final epoch (`results.csv`):

| Precision | Recall | mAP@50 | mAP@50–95 |
| ---: | ---: | ---: | ---: |
| 0.848 | 0.821 | 0.846 | 0.485 |

These are validation-split metrics, not a live-match benchmark. The friendly
class is rare (956 boxes vs 55,348 enemy boxes), so its recall is weaker.

## Troubleshooting

- **`CUDA available: False`** — install the CUDA 12.1 PyTorch wheel shown
  above; check `nvidia-smi`.
- **`CUDNN_STATUS_EXECUTION_FAILED`** — keep `amp=False` and the deterministic
  flags in `train.py`; update the NVIDIA driver.
- **Out of VRAM** — reduce `batch` (32 → 16 → 8) and/or `imgsz`.
- **Expired dataset URLs** — re-export the dataset manifest; signed CDN links
  are temporary.
- **ONNX export fails** — run `python export_onnx.py` from an environment with
  a working `onnx` package; check the Ultralytics export log for missing
  dependencies.

## What is not here

- Raw images/videos (4.1 GB) — not committed.
- Game assets, textures or audio — not committed.
- Aim/input automation — not implemented.

