"""
Vision AI Real-time Engine
Captures screen / region, runs YOLO FP16 inference on RTX 4070 Ti, and streams detections over UDP to Rust UI.
"""

import sys
import time
import socket
import json
import ctypes
from ctypes import wintypes
import numpy as np

# Проверяем наличие PyTorch и Ultralytics
try:
    import torch
    from ultralytics import YOLO
except ImportError:
    print("[ERROR] PyTorch or Ultralytics not installed in current environment.")
    sys.exit(1)

# Windows GDI Capture via ctypes for ultra-low latency (<1ms)
user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32

SRCCOPY = 0x00CC0020
CAPTUREBLT = 0x40000000

class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ('biSize', wintypes.DWORD),
        ('biWidth', wintypes.LONG),
        ('biHeight', wintypes.LONG),
        ('biPlanes', wintypes.WORD),
        ('biBitCount', wintypes.WORD),
        ('biCompression', wintypes.DWORD),
        ('biSizeImage', wintypes.DWORD),
        ('biXPelsPerMeter', wintypes.LONG),
        ('biYPelsPerMeter', wintypes.LONG),
        ('biClrUsed', wintypes.DWORD),
        ('biClrImportant', wintypes.DWORD)
    ]

class FastScreenCapturer:
    def __init__(self, capture_w=640, capture_h=640):
        self.capture_w = capture_w
        self.capture_h = capture_h
        self.screen_w = user32.GetSystemMetrics(0)
        self.screen_h = user32.GetSystemMetrics(1)
        
        self.left = (self.screen_w - self.capture_w) // 2
        self.top = (self.screen_h - self.capture_h) // 2
        
        self.h_screen_dc = user32.GetDC(0)
        self.h_mem_dc = gdi32.CreateCompatibleDC(self.h_screen_dc)
        self.h_bitmap = gdi32.CreateCompatibleBitmap(self.h_screen_dc, self.capture_w, self.capture_h)
        gdi32.SelectObject(self.h_mem_dc, self.h_bitmap)

        self.bmi = BITMAPINFOHEADER()
        self.bmi.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        self.bmi.biWidth = self.capture_w
        self.bmi.biHeight = -self.capture_h  # Верхний левый угол
        self.bmi.biPlanes = 1
        self.bmi.biBitCount = 24  # BGR
        self.bmi.biCompression = 0

        self.buffer = (ctypes.c_ubyte * (self.capture_w * self.capture_h * 3))()

    def grab(self):
        # BitBlt центра экрана
        gdi32.BitBlt(
            self.h_mem_dc, 0, 0, self.capture_w, self.capture_h,
            self.h_screen_dc, self.left, self.top,
            SRCCOPY
        )
        gdi32.GetDIBits(
            self.h_mem_dc, self.h_bitmap, 0, self.capture_h,
            ctypes.byref(self.buffer), ctypes.byref(self.bmi), 0
        )
        # Преобразуем в numpy массив HWC BGR
        img = np.frombuffer(self.buffer, dtype=np.uint8).reshape((self.capture_h, self.capture_w, 3))
        return img, self.left, self.top

    def close(self):
        gdi32.DeleteObject(self.h_bitmap)
        gdi32.DeleteDC(self.h_mem_dc)
        user32.ReleaseDC(0, self.h_screen_dc)


def main():
    print("=" * 60)
    print("Starting Vision Real-time AI Engine (CUDA RTX 4070 Ti)")
    print("=" * 60)

    device = 0 if torch.cuda.is_available() else "cpu"
    print(f"[ENGINE] Device: {device} ({torch.cuda.get_device_name(0) if torch.cuda.is_available() else 'CPU'})")

    # Ищем лучшую обученную модель
    import os
    model_paths = [
        "best.pt",
        "../train_python/runs/apex_yolo/weights/best.pt",
        "enemy_detector.onnx"
    ]
    model_path = None
    for p in model_paths:
        if os.path.exists(p):
            model_path = p
            break

    if not model_path:
        print("[ERROR] No trained YOLO weights found!")
        sys.exit(1)

    print(f"[ENGINE] Loading model: {model_path} ...")
    model = YOLO(model_path)
    if device == 0:
        try:
            model.to('cuda').half()
            print("[ENGINE] Model accelerated on CUDA RTX 4070 Ti (FP16 Tensor Cores)")
        except Exception as e:
            print(f"[WARN] Failed to set FP16 on CUDA: {e}")
    print("[ENGINE] Model loaded successfully.")

    # UDP сокет для отправки детекций в Rust UI
    udp_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    dest_addr = ("127.0.0.1", 44555)

    # UDP сокет для приема команд от Rust UI (пороги, настройки)
    cmd_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    cmd_sock.bind(("127.0.0.1", 44556))
    cmd_sock.setblocking(False)

    conf_threshold = 0.50
    iou_threshold = 0.45
    max_det = 15

    capturer = FastScreenCapturer(640, 640)
    print(f"[ENGINE] Capturing {capturer.capture_w}x{capturer.capture_h} at center ({capturer.left}, {capturer.top})")
    print(f"[ENGINE] Streaming to Rust overlay at 127.0.0.1:44555...")

    fps_count = 0
    fps_start = time.time()
    current_fps = 60.0

    try:
        while True:
            # 1. Проверяем новые команды от Rust
            try:
                data, _ = cmd_sock.recvfrom(2048)
                cmd = json.loads(data.decode('utf-8'))
                if "conf" in cmd:
                    conf_threshold = float(cmd["conf"])
                if "iou" in cmd:
                    iou_threshold = float(cmd["iou"])
                if "max_det" in cmd:
                    max_det = int(cmd["max_det"])
                if "stop" in cmd and cmd["stop"]:
                    print("[ENGINE] Received stop command.")
                    break
            except BlockingIOError:
                pass
            except Exception:
                pass

            t0 = time.perf_counter()
            frame, left, top = capturer.grab()
            t_cap = (time.perf_counter() - t0) * 1000.0

            # 2. Инференс нейросети YOLO на GPU
            t1 = time.perf_counter()
            results = model.predict(
                source=frame,
                imgsz=640,
                conf=conf_threshold,
                iou=iou_threshold,
                max_det=max_det,
                device=device,
                verbose=False
            )
            t_infer = (time.perf_counter() - t1) * 1000.0

            boxes_payload = []
            if results and len(results) > 0:
                res = results[0]
                if res.boxes is not None and len(res.boxes) > 0:
                    for b in res.boxes:
                        xyxy = b.xyxy[0].cpu().numpy()
                        cls_id = int(b.cls[0].item())
                        conf = float(b.conf[0].item())

                        # Переводим локальные координаты 640x640 в глобальные координаты экрана
                        x1 = float(xyxy[0]) + left
                        y1 = float(xyxy[1]) + top
                        x2 = float(xyxy[2]) + left
                        y2 = float(xyxy[3]) + top
                        w = x2 - x1
                        h = y2 - y1
                        center_x = x1 + w / 2.0
                        center_y = y1 + h / 2.0

                        boxes_payload.append({
                            "cls": cls_id,
                            "conf": round(conf, 3),
                            "x": round(center_x, 1),
                            "y": round(center_y, 1),
                            "w": round(w, 1),
                            "h": round(h, 1)
                        })

            fps_count += 1
            if time.time() - fps_start >= 0.5:
                current_fps = fps_count / (time.time() - fps_start)
                fps_count = 0
                fps_start = time.time()

            # 3. Отправка пакета в Rust UI
            packet = {
                "fps": round(current_fps, 1),
                "cap_ms": round(t_cap, 2),
                "infer_ms": round(t_infer, 2),
                "render_ms": 0.4,
                "screen_w": capturer.screen_w,
                "screen_h": capturer.screen_h,
                "targets": boxes_payload
            }
            msg = json.dumps(packet).encode('utf-8')
            udp_sock.sendto(msg, dest_addr)

            # Пейсинг частоты кадров до ~144 FPS для разгрузки CPU/GPU
            dt = time.perf_counter() - t0
            if dt < 0.0069:
                time.sleep(0.0069 - dt)

    except KeyboardInterrupt:
        print("\n[ENGINE] Stopped by user.")
    finally:
        capturer.close()
        udp_sock.close()
        cmd_sock.close()

if __name__ == "__main__":
    main()

