import os
import sys
from pathlib import Path
import torch
from ultralytics import YOLO

# 1. Настройки стабильности CUDA / cuDNN для Windows и RTX 40-й серии
torch.backends.cudnn.benchmark = False       # Отключаем автоподбор ядер cuDNN (предотвращает CUDNN_STATUS_EXECUTION_FAILED)
torch.backends.cudnn.deterministic = True   # Детерминированный режим для стабильности
torch.backends.cuda.matmul.allow_tf32 = True # Включаем TensorFloat-32 (аппаратное ускорение на RTX 4070 Ti)
torch.backends.cudnn.allow_tf32 = True

def main():
    print("=" * 60)
    print("Запуск стабильного обучения YOLO (cuDNN Safe Mode)")
    print("=" * 60)

    base_dir = Path(__file__).resolve().parent
    dataset_file = Path(os.environ.get("DATASET", base_dir.parent / "apex-2.ndjson"))

    if not dataset_file.exists():
        print(f"[ОШИБКА] Файл датасета не найден по пути: {dataset_file}")
        sys.exit(1)

    print(f"Датасет: {dataset_file}")

    # Проверка доступности GPU
    if torch.cuda.is_available():
        device = 0
        gpu_name = torch.cuda.get_device_name(0)
        vram_gb = torch.cuda.get_device_properties(0).total_memory / (1024 ** 3)
        print(f"Устройство: {gpu_name} ({vram_gb:.2f} GB VRAM)")
    else:
        device = "cpu"
        print("[ПРЕДУПРЕЖДЕНИЕ] CUDA недоступна, запуск на CPU.")

    # Загружаем базовую модель yolo26n
    model_path = base_dir / "yolo26n.pt"
    if model_path.exists():
        print(f"Загрузка локальных весов: {model_path}")
        model = YOLO(str(model_path))
    else:
        print("Загрузка модели YOLO26...")
        model = YOLO("yolo26n.pt")

    project_dir = base_dir / "runs"
    
    print("\nЗапуск обучения...")
    print("Датасет уже скачан на диск, повторной загрузки 4 ГБ не потребуется!\n")

    results = model.train(
        data=str(dataset_file),
        epochs=30,
        imgsz=640,
        batch=32,            # 16 отлично укладывается в память 12GB (занято всего ~2.5 ГБ)
        device=device,
        workers=2,           # 2 воркера оптимальны и стабильны для Windows
        amp=False,           # Отключаем AMP Scaler (устраняет сбой backward pass cuDNN)
        patience=10,
        project=str(project_dir),
        name="apex_yolo",
        exist_ok=True,
        save=True
    )

    print("\n" + "=" * 60)
    print("Обучение успешно завершено!")
    best_weights = project_dir / "apex_yolo" / "weights" / "best.pt"
    print(f"Лучшие веса сохранены в: {best_weights}")
    print("Запустите export_onnx.py для получения модели для Rust.")
    print("=" * 60)

if __name__ == "__main__":
    main()
