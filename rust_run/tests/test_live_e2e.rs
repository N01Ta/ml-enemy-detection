use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use procmod_overlay::{Color, Overlay, OverlayTarget};
use ultralytics_inference::{Device, InferenceConfig, YOLOModel};

#[link(name = "user32")]
extern "system" {
    fn GetDesktopWindow() -> *mut std::ffi::c_void;
    fn SetWindowDisplayAffinity(hWnd: *mut std::ffi::c_void, dwAffinity: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryA(lpPathName: *const std::ffi::c_char) -> i32;
}

#[path = "../src/native_engine.rs"]
mod native_engine;
use native_engine::NativeScreenCapturer;

#[derive(Clone, Default)]
pub struct LiveTargetBox {
    pub cls: usize,
    pub conf: f32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Default)]
pub struct LiveDetectionPacket {
    pub fps: f32,
    pub cap_ms: f32,
    pub infer_ms: f32,
    pub render_ms: f32,
    pub screen_w: i32,
    pub screen_h: i32,
    pub targets: Vec<LiveTargetBox>,
}

#[test]
fn test_live_e2e_latency() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    let trt_lib = "C:\\Users\\N01Ta\\Desktop\\w\\ai\\rust_run";
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{};{};{}", trt_lib, "C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    let _ = ort::init_from(dll_path.to_str().unwrap()).map(|b| b.commit());

    let desktop_hwnd = unsafe { GetDesktopWindow() } as isize;
    let is_running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&is_running);

    // 1. Overlay thread
    let overlay_thread = std::thread::spawn(move || {
        let mut overlay = match Overlay::new(OverlayTarget::Hwnd(desktop_hwnd)) {
            Ok(o) => o,
            Err(e) => {
                println!("Failed to create overlay: {:?}", e);
                return;
            }
        };

        unsafe {
            let hwnd = overlay.hwnd() as *mut std::ffi::c_void;
            let aff = SetWindowDisplayAffinity(hwnd, 0x00000011);
            println!("Overlay HWND: {:p}, SetWindowDisplayAffinity: {}", hwnd, aff);
        }

        while running_clone.load(Ordering::Relaxed) {
            if overlay.begin_frame().is_ok() {
                overlay.circle(500.0, 500.0, 150.0, Color::CYAN);
                overlay.rect(300.0, 300.0, 100.0, 200.0, Color::RED);
                overlay.text(300.0, 280.0, "Apex Enemy [98%]", 14.0, Color::WHITE);
                let _ = overlay.end_frame();
            }
            std::thread::sleep(Duration::from_millis(5)); // ~180 FPS pacing
        }
    });

    std::thread::sleep(Duration::from_millis(100));

    // 2. Load TensorRT YOLO model
    let trt_config = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0));

    let mut model = YOLOModel::load_with_config("enemy_detector.onnx", trt_config)
        .expect("Failed to load TensorRT model");

    let mut capturer = NativeScreenCapturer::new();

    // Warmup
    let _ = model.predict_image(capturer.capture_to_image(), "warmup".into());

    let iters = 50;
    let mut cap_times = Vec::new();
    let mut infer_times = Vec::new();
    let mut total_times = Vec::new();

    for i in 0..iters {
        let t_loop_start = Instant::now();

        let t_cap = Instant::now();
        let dyn_img = capturer.capture_to_image();
        let cap_ms = t_cap.elapsed().as_secs_f64() * 1000.0;
        cap_times.push(cap_ms);

        let t_infer = Instant::now();
        let _ = model.predict_image(dyn_img, "screen".into());
        let infer_ms = t_infer.elapsed().as_secs_f64() * 1000.0;
        infer_times.push(infer_ms);

        let total_ms = t_loop_start.elapsed().as_secs_f64() * 1000.0;
        total_times.push(total_ms);

        if i < 5 || i % 10 == 0 {
            println!(
                "Iteration {}: Cap = {:.2} ms | Infer = {:.2} ms | Total = {:.2} ms ({:.0} FPS)",
                i,
                cap_ms,
                infer_ms,
                total_ms,
                1000.0 / total_ms
            );
        }
    }

    is_running.store(false, Ordering::Relaxed);
    let _ = overlay_thread.join();

    let avg_cap = cap_times.iter().sum::<f64>() / cap_times.len() as f64;
    let avg_infer = infer_times.iter().sum::<f64>() / infer_times.len() as f64;
    let avg_total = total_times.iter().sum::<f64>() / total_times.len() as f64;

    println!("\n==========================================");
    println!(">>> FULL END-TO-END PIPELINE BENCHMARK <<<");
    println!(">>> Average Capture Latency:   {:.3} ms", avg_cap);
    println!(">>> Average Inference Latency: {:.3} ms", avg_infer);
    println!(">>> Average Total Frame Time:  {:.3} ms", avg_total);
    println!(">>> SUSTAINED FRAME RATE:      {:.1} FPS", 1000.0 / avg_total);
    println!("==========================================\n");
}

