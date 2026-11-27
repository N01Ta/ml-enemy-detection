use std::io::Write;
use ultralytics_inference::{Device, InferenceConfig, YOLOModel};

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryA(lpPathName: *const std::ffi::c_char) -> i32;
}

#[test]
fn test_cuda_and_tensorrt() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    
    // Also prepend to PATH
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib;{}", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    eprintln!(">>> Loading ORT dynamically from: {:?}", dll_path);
    let _ = ort::init_from(dll_path.to_str().unwrap()).expect("Failed to load onnxruntime.dll").commit();
    std::io::stderr().flush().unwrap();

    let test_img = image::open("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\datasets\\apex-2-53af0290\\images\\train\\0000a712158fa0387c48692e2e306a6a_841.jpg")
        .unwrap_or_else(|_| image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640)));

    // ==========================================
    // 1. BENCHMARK CUDA
    // ==========================================
    eprintln!("\n==========================================");
    eprintln!("--- TESTING DEVICE::CUDA(0) ---");
    eprintln!("==========================================");
    let cuda_config = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::Cuda(0));

    match YOLOModel::load_with_config("enemy_detector.onnx", cuda_config) {
        Ok(mut m) => {
            eprintln!("[SUCCESS] Model loaded on CUDA(0)!");
            // Warmup
            let _ = m.predict_image(&test_img, "test".into());

            let start = std::time::Instant::now();
            let iters = 20;
            for _ in 0..iters {
                let _ = m.predict_image(&test_img, "test".into());
            }
            let avg_ms = (start.elapsed().as_secs_f32() * 1000.0) / (iters as f32);
            eprintln!(">>> [CUDA(0) BENCHMARK] Latency: {:.2} ms | FPS: {:.1} <<<", avg_ms, 1000.0 / avg_ms);
        }
        Err(e) => {
            eprintln!("[CUDA ERROR] Could not load model on CUDA(0): {:?}", e);
        }
    }

    // ==========================================
    // 2. BENCHMARK TENSORRT
    // ==========================================
    eprintln!("\n==========================================");
    eprintln!("--- TESTING DEVICE::TENSORRT(0) ---");
    eprintln!("==========================================");
    let trt_config = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0));

    match YOLOModel::load_with_config("enemy_detector.onnx", trt_config) {
        Ok(mut m) => {
            eprintln!("[SUCCESS] Model loaded on TensorRT(0)!");
            // Warmup
            let _ = m.predict_image(&test_img, "test".into());

            let start = std::time::Instant::now();
            let iters = 20;
            for _ in 0..iters {
                let _ = m.predict_image(&test_img, "test".into());
            }
            let avg_ms = (start.elapsed().as_secs_f32() * 1000.0) / (iters as f32);
            eprintln!(">>> [TensorRT(0) BENCHMARK] Latency: {:.2} ms | FPS: {:.1} <<<", avg_ms, 1000.0 / avg_ms);
        }
        Err(e) => {
            eprintln!("[TensorRT ERROR] Could not load model on TensorRT(0): {:?}", e);
        }
    }
}

#[test]
fn test_live_pipeline_fps() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib;{}", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    let _ = ort::init_from(dll_path.to_str().unwrap()).map(|b| b.commit());

    let cuda_config = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::Cuda(0));

    if let Ok(mut m) = YOLOModel::load_with_config("enemy_detector.onnx", cuda_config) {
        let test_img = image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640));
        let _ = m.predict_image(&test_img, "warmup".into());

        let start = std::time::Instant::now();
        let iters = 50;
        for _ in 0..iters {
            let _ = m.predict_image(&test_img, "screen".into());
        }
        let total_time = start.elapsed();
        let avg_ms = (total_time.as_secs_f32() * 1000.0) / (iters as f32);
        let real_fps = (iters as f32) / total_time.as_secs_f32();

        eprintln!("\n==========================================");
        eprintln!(">>> LIVE FULL INFERENCE PIPELINE <<<");
        eprintln!(">>> Latency per frame: {:.2} ms", avg_ms);
        eprintln!(">>> REAL-TIME PIPELINE SPEED: {:.1} FPS <<<", real_fps);
        eprintln!("==========================================\n");
    }
}

