import sys
import torch

def main():
    print(f"Python version: {sys.version.split()[0]}")
    print(f"PyTorch version: {torch.__version__}")
    
    cuda_available = torch.cuda.is_available()
    print(f"CUDA available: {cuda_available}")
    
    if cuda_available:
        gpu_count = torch.cuda.device_count()
        gpu_name = torch.cuda.get_device_name(0)
        vram_gb = torch.cuda.get_device_properties(0).total_memory / (1024 ** 3)
        print(f"GPU: {gpu_name} (Всего устройств: {gpu_count})")
        print(f"VRAM: {vram_gb:.2f} GB")
        print("\n[OK] Видеокарта готова к обучению модели на GPU!")
    else:
        print("\n[ВНИМАНИЕ] CUDA не обнаружена. Обучение на процессоре (CPU) будет очень медленным.")
        print("Убедитесь, что установлена версия PyTorch с поддержкой CUDA:")
        print("pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121")

if __name__ == "__main__":
    main()
