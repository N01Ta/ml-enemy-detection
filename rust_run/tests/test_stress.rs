use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use procmod_overlay::{Color, Overlay, OverlayTarget};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};
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

struct SharedFrameData {
    rgb: image::DynamicImage,
    new_frame_counter: u64,
    last_cap_ms: f32,
    screen_w: i32,
    screen_h: i32,
}

struct WgcHandler {
    shared_data: Arc<Mutex<SharedFrameData>>,
    rgb_temp: Vec<u8>,
}

impl GraphicsCaptureApiHandler for WgcHandler {
    type Flags = Arc<Mutex<SharedFrameData>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let shared_data = ctx.flags;
        Ok(Self {
            shared_data,
            rgb_temp: vec![0u8; 640 * 640 * 3],
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let t0 = Instant::now();
        let w = frame.width();
        let h = frame.height();

        if w < 640 || h < 640 {
            return Ok(());
        }

        let crop_w = 640;
        let crop_h = 640;
        let start_x = (w.saturating_sub(crop_w)) / 2;
        let start_y = (h.saturating_sub(crop_h)) / 2;
        let end_x = start_x + crop_w;
        let end_y = start_y + crop_h;

        let mut crop_buf = match frame.buffer_crop(start_x, start_y, end_x, end_y) {
            Ok(b) => b,
            Err(_) => return Ok(()),
        };
        let row_pitch = crop_buf.row_pitch() as usize;
        let raw = crop_buf.as_raw_buffer();

        if row_pitch < 640 * 4 || raw.len() < 640 * row_pitch {
            return Ok(());
        }

        for y in 0..640 {
            let row_start = y * row_pitch;
            let src_slice = &raw[row_start..row_start + 640 * 4];
            let dst_offset = y * 640 * 3;
            let dst_slice = &mut self.rgb_temp[dst_offset..dst_offset + 640 * 3];
            for x in 0..640 {
                dst_slice[x * 3] = src_slice[x * 4];
                dst_slice[x * 3 + 1] = src_slice[x * 4 + 1];
                dst_slice[x * 3 + 2] = src_slice[x * 4 + 2];
            }
        }

        let cap_ms = t0.elapsed().as_secs_f32() * 1000.0;

        if let Ok(mut lock) = self.shared_data.lock() {
            if let Some(rgb) = lock.rgb.as_mut_rgb8() {
                rgb.copy_from_slice(&self.rgb_temp);
            }
            lock.new_frame_counter += 1;
            lock.last_cap_ms = cap_ms;
            lock.screen_w = w as i32;
            lock.screen_h = h as i32;
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn test_stress_wgc_trt_overlay() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    let trt_lib = "C:\\Users\\N01Ta\\Desktop\\w\\ai\\rust_run";
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{};{};{}", trt_lib, "C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    let _ = ort::init_from(dll_path.to_str().unwrap()).map(|b| b.commit());

    let is_running = Arc::new(AtomicBool::new(true));
    let running_overlay = Arc::clone(&is_running);

    // 1. Overlay Thread
    let desktop_hwnd = unsafe { GetDesktopWindow() } as isize;
    let overlay_thread = std::thread::spawn(move || {
        let mut overlay = match Overlay::new(OverlayTarget::Hwnd(desktop_hwnd)) {
            Ok(o) => o,
            Err(e) => {
                println!("Overlay error: {:?}", e);
                return;
            }
        };
        unsafe {
            let hwnd = overlay.hwnd() as *mut std::ffi::c_void;
            SetWindowDisplayAffinity(hwnd, 0x00000011);
        }
        while running_overlay.load(Ordering::Relaxed) {
            if overlay.begin_frame().is_ok() {
                overlay.circle(600.0, 600.0, 100.0, Color::CYAN);
                let _ = overlay.end_frame();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    // 2. WGC Streamer
    let real_img = image::open("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\datasets\\apex-2-53af0290\\images\\train\\0000a712158fa0387c48692e2e306a6a_841.jpg")
        .map(|img| img.resize_exact(640, 640, image::imageops::FilterType::Nearest))
        .unwrap_or_else(|_| image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640)));
    let shared_frame = Arc::new(Mutex::new(SharedFrameData {
        rgb: real_img,
        new_frame_counter: 0,
        last_cap_ms: 0.35,
        screen_w: 3440,
        screen_h: 1440,
    }));

    let monitor = Monitor::primary().unwrap();
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        Arc::clone(&shared_frame),
    );
    let mut wgc_ctrl = WgcHandler::start_free_threaded(settings).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // 3. TensorRT Model with cuda_preprocess enabled
    let trt_cfg = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0));

    let mut model = YOLOModel::load_with_config("enemy_detector.onnx", trt_cfg)
        .expect("Failed to load TensorRT model");

    println!("Starting 300 iterations of full concurrent pipeline (WGC + TensorRT + D3D11 Overlay)...");
    let mut local_dyn_img = image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640));

    for i in 0..300 {
        if let Ok(lock) = shared_frame.lock() {
            local_dyn_img = lock.rgb.clone();
        }
        let t0 = Instant::now();
        let _ = model.predict_image(&local_dyn_img, "stress".into()).expect("predict_image failed");
        let infer_ms = t0.elapsed().as_secs_f32() * 1000.0;

        if i % 50 == 0 {
            println!("Iteration {}: infer = {:.2} ms", i, infer_ms);
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    println!("Stress test completed successfully without crashes!");

    is_running.store(false, Ordering::Relaxed);
    let _ = overlay_thread.join();
    let _ = wgc_ctrl.stop();
}

