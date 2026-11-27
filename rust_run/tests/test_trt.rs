use ultralytics_inference::{InferenceConfig, YOLOModel, Device};

#[test]
fn test_trt_loading() {
    println!("Testing TensorRT provider...");
    let trt_lib = "C:\\Users\\N01Ta\\Desktop\\w\\ai\\rust_run";
    let torch_lib = "C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib";
    
    let cur_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{};{};{}", trt_lib, torch_lib, cur_path));

    println!("--- Testing TensorRT with cuda_preprocess(false) ---");
    let trt_cfg_safe = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0))
        .with_cuda_preprocess(false);

    match YOLOModel::load_with_config("enemy_detector.onnx", trt_cfg_safe) {
        Ok(mut m) => {
            println!("SUCCESS: Safe TensorRT loaded!");
            let img = image::DynamicImage::new_rgb8(640, 640);
            let _ = m.predict_image(&img, "bench".to_string());
            let start = std::time::Instant::now();
            let iters = 100;
            for _ in 0..iters {
                let _ = m.predict_image(&img, "bench".to_string());
            }
            let avg_ms = (start.elapsed().as_secs_f64() * 1000.0) / iters as f64;
            println!("Avg Safe TensorRT inference (100 iters): {:.2} ms!", avg_ms);
        }
        Err(e) => println!("ERROR loading safe TensorRT: {:#?}", e),
    }

    println!("\n--- Testing TensorRT with cuda_preprocess(true) ---");
    let trt_cfg_pre = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0))
        .with_cuda_preprocess(true);

    match YOLOModel::load_with_config("enemy_detector.onnx", trt_cfg_pre) {
        Ok(mut m) => {
            println!("SUCCESS: CUDA Preprocess TensorRT loaded!");
            let img = image::DynamicImage::new_rgb8(640, 640);
            let _ = m.predict_image(&img, "bench".to_string());
            let start = std::time::Instant::now();
            let iters = 100;
            for _ in 0..iters {
                let _ = m.predict_image(&img, "bench".to_string());
            }
            let avg_ms = (start.elapsed().as_secs_f64() * 1000.0) / iters as f64;
            println!("Avg CUDA Preprocess TensorRT inference (100 iters): {:.2} ms!", avg_ms);
        }
        Err(e) => println!("ERROR loading CUDA Preprocess TensorRT: {:#?}", e),
    }
}

