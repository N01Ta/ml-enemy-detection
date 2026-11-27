use ultralytics_inference::{Device, InferenceConfig, YOLOModel};

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryA(lpPathName: *const std::ffi::c_char) -> i32;
}

#[test]
fn test_trt_without_cuda_preprocess() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib;{}", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    let _ = ort::init_from(dll_path.to_str().unwrap()).map(|b| b.commit());

    let mut img_buf = image::RgbImage::new(640, 640);
    for (x, y, pixel) in img_buf.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8]);
    }
    let test_img = image::DynamicImage::ImageRgb8(img_buf);

    let trt_cfg = InferenceConfig::new()
        .with_confidence(0.25)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_rect(false)
        .with_cuda_preprocess(false)
        .with_device(Device::TensorRt(0));

    eprintln!("Loading TensorRT model with with_cuda_preprocess(false)...");
    let mut model = YOLOModel::load_with_config("enemy_detector.onnx", trt_cfg).expect("load model");

    let iters = 50;
    let start = std::time::Instant::now();
    for i in 0..iters {
        let _ = model.predict_image(&test_img, format!("f_{}", i)).expect("predict");
    }
    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_secs_f32() * 1000.0 / (iters as f32);
    let fps = (iters as f32) / elapsed.as_secs_f32();
    eprintln!(">>> TensorRT with_cuda_preprocess(false): {:.2} ms per frame | {:.1} FPS <<<", avg_ms, fps);
}

