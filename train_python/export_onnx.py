import shutil
import sys
from pathlib import Path
from ultralytics import YOLO

def main():
    print("=" * 60)
    print("Экспорт обученной модели PyTorch (.pt) в формат ONNX (.onnx)")
    print("=" * 60)

    base_dir = Path(__file__).resolve().parent
    weights_path = base_dir / "runs" / "apex_yolo" / "weights" / "best.pt"

    if not weights_path.exists():
        print(f"[ОШИБКА] Файл весов не найден: {weights_path}")
        print("Сначала запустите обучение через train.py")
        sys.exit(1)

    print(f"Загрузка весов: {weights_path}")
    model = YOLO(str(weights_path))

    # Экспорт в ONNX
    print("\nЭкспортируем в формат ONNX...")
    exported_path = model.export(
        format="onnx",
        half=True,       # FP16 точность (в 2 раза быстрее на GPU и занимает меньше памяти)
        dynamic=False,   # Фиксированный размер 640x640 для максимальной скорости в ONNX Runtime
        imgsz=640
    )

    print(f"\n[УСПЕХ] Модель успешно экспортирована: {exported_path}")

    # Копируем модель в папку rust_run для приложения на Rust
    rust_run_dir = base_dir.parent / "rust_run"
    rust_run_dir.mkdir(parents=True, exist_ok=True)
    target_onnx = rust_run_dir / "enemy_detector.onnx"

    shutil.copy2(exported_path, target_onnx)
    print(f"[СКОПИРОВАНО] Модель скопирована для Rust: {target_onnx}")
    print("=" * 60)

if __name__ == "__main__":
    main()

