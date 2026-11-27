# Third-party notices

This repository contains original code and third-party components. The MIT
license in [LICENSE](LICENSE) applies to the original code only, unless a file
states otherwise.

## Vendored components

| Component | Location | License |
| --- | --- | --- |
| `procmod-overlay` 3.0.1 | `rust_run/vendor/procmod-overlay/` | MIT — see the vendored `LICENSE` |
| `Roboto-Regular.ttf` (inside procmod-overlay) | `rust_run/vendor/procmod-overlay/assets/` | Apache License 2.0 |

## Dependencies

- **Ultralytics YOLO / YOLO26n** — training and export code depends on
  [Ultralytics](https://github.com/ultralytics/ultralytics), which is licensed
  under AGPL-3.0. The trained weights in this repository are a model artifact
  produced with that toolchain; review the upstream license before
  redistributing the model or using it commercially.
- **ONNX Runtime / TensorRT / CUDA / DirectML** — runtime libraries retain
  their own licenses and are not vendored here.
- **windows-capture, ort, ultralytics-inference, egui/eframe** — Rust crates
  retain their own licenses; see `Cargo.lock` and the crates.io pages.

## Game assets and trademarks

Apex Legends, EA, Respawn Entertainment and all related names, assets and
trademarks belong to their respective owners. This project is not affiliated
with or endorsed by EA or Respawn. No game models, textures or audio are
redistributed. The demo video/screenshots are included for documentation and
research only. The original 4.1 GB training dataset, its manifest and signed
CDN URLs are not included in this repository.

The demo video and screenshots contain gameplay footage captured by the author
for an educational computer-vision experiment. Do not use this project in
online matches; visual overlays/ESP violate EA's Terms of Service.

## Model weights

The model weights are provided as-is for demonstration and research. They are
not a commercial product and are not warranted for any particular use,
accuracy or fitness. The friendly class is heavily underrepresented in the
training data.
