# Rust runtime

Windows-only Rust application that captures the screen, runs YOLO26n through
ONNX Runtime / TensorRT, and renders a transparent click-through overlay.

## Prerequisites

- Windows 10/11 x64
- Rust stable + MSVC build tools + Windows SDK
- ONNX Runtime DLL (`onnxruntime.dll`)
- Optional: NVIDIA GPU, CUDA 12.x, cuDNN, TensorRT 10.x
- The runtime model: `enemy_detector.onnx` (included)

## Build & run

```powershell
cd rust_run
$env:ORT_DYLIB_PATH = "C:\path\to\onnxruntime.dll"
$env:TENSORRT_DIR   = "C:\path\to\TensorRT-10.x"   # optional
cargo run --release
```

The app searches for `onnxruntime.dll` in this order:

1. `ORT_DYLIB_PATH`
2. `onnxruntime.dll` in the current directory
3. `onnxruntime.dll` next to the executable
4. `onnxruntime.dll` in `TENSORRT_DIR`, `TENSORRT_DIR\bin`, `TENSORRT_DIR\lib`

The GPU build of ONNX Runtime is required for CUDA/TensorRT. A CPU-only DLL
will fall back to CPU regardless of the selected provider.

## Inference providers

The UI selects a provider; the native runner attempts them in this order:

```text
TensorRT -> CUDA -> DirectML -> CPU
```

`TensorRT` and `CUDA` need the NVIDIA runtime DLLs on `PATH` / in
`TENSORRT_DIR`. `DirectML` uses the DirectML EP; `CPU` is the slow fallback.

## Model and configs

- `enemy_detector.onnx` — FP16, fixed 640×640, 2 classes.
- `configs/Default.json` — balanced visual preset.
- `configs/Precise.json` — tighter FOV / higher confidence.
- `configs/HighRecall.json` — filled-box / high-recall visual preset.

All three profiles are visual presets. They do not enable aim, input or game
memory access. The app writes new profiles to `configs/`.

## Overlay

The overlay is a separate topmost Direct3D 11 window rendered through the
vendored `procmod-overlay` crate. It is click-through and does not inject into
or read from the game process.

The overlay calls `SetWindowDisplayAffinity(..., WDA_EXCLUDEFROMCAPTURE)`, so
desktop capture tools (OBS, ShadowPlay, Game Bar) may not record it. The demo
GIF in the repository was generated offline from the included ONNX model; see
the root README.

## Python prototype

`vision_engine.py` is an experimental standalone engine:

```text
GDI capture -> ultralytics YOLO -> JSON over UDP 127.0.0.1:44555
```

The current Rust app uses the native WGC/inference path and does **not** bind a
UDP receiver. The Python script is kept as a reference/experiment; it is not a
second supported runtime.

## Tests

```powershell
cargo test --release
```

There are 17 test files. Several are hardware/display/live-window probes:

- `test_live_e2e`, `test_wgc*`, `test_dxgi_debug`, `test_enum_dxgi` need a
  desktop/session context.
- `test_trt*` need a TensorRT-capable setup.
- `test_procmod`, `test_model`, `test_tracking`, `test_affinity` are more
  self-contained but still assume Windows.

Run them selectively; this repository has no CI matrix.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `onnxruntime.dll` load error | `ORT_DYLIB_PATH` points to a matching GPU/CPU DLL |
| TensorRT provider fails | TensorRT 10.x installed; `TENSORRT_DIR` set; CUDA/cuDNN on `PATH` |
| CUDA provider fails | NVIDIA driver + CUDA 12.x + GPU ONNX Runtime build |
| No detections | correct model, confidence threshold, center crop / display size |
| Overlay invisible in recording | `WDA_EXCLUDEFROMCAPTURE`; use an external capture or the offline preview |
| App runs but slow | DirectML/CPU fallback; check the provider in the UI |

## Safety

This runtime is a visual POC. It contains no aim automation, no input
emulation, no game-memory access and no anti-cheat bypass. Do not use it in
online matches.

