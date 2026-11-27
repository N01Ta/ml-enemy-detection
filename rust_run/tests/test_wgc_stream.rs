use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};
use ultralytics_inference::{Device, InferenceConfig, YOLOModel};

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryA(lpPathName: *const std::ffi::c_char) -> i32;
}

struct SharedFrameData {
    rgb: image::DynamicImage,
    new_frame_counter: u64,
    last_cap_ms: f32,
}

struct WgcHandler {
    shared_data: Arc<Mutex<SharedFrameData>>,
    fov_size: Arc<AtomicU32>,
    rgb_temp: Vec<u8>,
}

impl GraphicsCaptureApiHandler for WgcHandler {
    type Flags = (Arc<Mutex<SharedFrameData>>, Arc<AtomicU32>);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (shared_data, fov_size) = ctx.flags;
        Ok(Self {
            shared_data,
            fov_size,
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

        let crop_w = 640;
        let crop_h = 640;
        let start_x = (w - crop_w) / 2;
        let start_y = (h - crop_h) / 2;
        let end_x = start_x + crop_w;
        let end_y = start_y + crop_h;

        let mut crop_buf = frame.buffer_crop(start_x, start_y, end_x, end_y)?;
        let row_pitch = crop_buf.row_pitch() as usize;
        let raw = crop_buf.as_raw_buffer();

        // Convert RGBA8 to RGB24
        let dst_ptr = self.rgb_temp.as_mut_ptr();
        for y in 0..640 {
            let src_row = unsafe { raw.as_ptr().add(y * row_pitch) };
            let dst_row = unsafe { dst_ptr.add(y * 640 * 3) };
            for x in 0..640 {
                unsafe {
                    let s = src_row.add(x * 4);
                    let d = dst_row.add(x * 3);
                    *d = *s;
                    *d.add(1) = *s.add(1);
                    *d.add(2) = *s.add(2);
                }
            }
        }

        let cap_ms = t0.elapsed().as_secs_f32() * 1000.0;

        if let Ok(mut lock) = self.shared_data.lock() {
            if let Some(rgb) = lock.rgb.as_mut_rgb8() {
                rgb.copy_from_slice(&self.rgb_temp);
            }
            lock.new_frame_counter += 1;
            lock.last_cap_ms = cap_ms;
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn test_wgc_streaming_inference() {
    let torch_lib = std::ffi::CString::new("C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib").unwrap();
    unsafe {
        SetDllDirectoryA(torch_lib.as_ptr());
    }
    let trt_lib = "C:\\Users\\N01Ta\\Desktop\\w\\ai\\rust_run";
    let current_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{};{};{}", trt_lib, "C:\\Users\\N01Ta\\Desktop\\w\\ai\\train_python\\.venv\\Lib\\site-packages\\torch\\lib", current_path));

    let dll_path = std::env::current_dir().unwrap().join("onnxruntime.dll");
    let _ = ort::init_from(dll_path.to_str().unwrap()).map(|b| b.commit());

    let shared_frame = Arc::new(Mutex::new(SharedFrameData {
        rgb: image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640)),
        new_frame_counter: 0,
        last_cap_ms: 0.0,
    }));
    let fov_size = Arc::new(AtomicU32::new(640));

    let monitor = Monitor::primary().unwrap();
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (Arc::clone(&shared_frame), Arc::clone(&fov_size)),
    );

    println!("Starting Windows Graphics Capture background streamer...");
    let capture_control = WgcHandler::start_free_threaded(settings).unwrap();

    // Wait for first frame
    std::thread::sleep(Duration::from_millis(100));

    let trt_config = InferenceConfig::new()
        .with_confidence(0.5)
        .with_iou(0.45)
        .with_imgsz(640, 640)
        .with_device(Device::TensorRt(0));

    let mut model = YOLOModel::load_with_config("enemy_detector.onnx", trt_config)
        .expect("Failed to load TensorRT model");

    println!("TensorRT model loaded. Running 30 inference iterations on WGC stream...");

    let mut total_times = Vec::new();
    let mut cap_times = Vec::new();
    let mut infer_times = Vec::new();

    let mut local_img = image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640));

    for i in 0..30 {
        let t_start = Instant::now();

        let t_cap = Instant::now();
        let cap_ms = if let Ok(lock) = shared_frame.lock() {
            local_img = lock.rgb.clone();
            lock.last_cap_ms
        } else {
            0.0
        };
        let cap_fetch_time = t_cap.elapsed().as_secs_f64() * 1000.0;
        cap_times.push(cap_ms as f64);

        let t_infer = Instant::now();
        let _ = model.predict_image(&local_img, "screen".into());
        let infer_ms = t_infer.elapsed().as_secs_f64() * 1000.0;
        infer_times.push(infer_ms);

        let total_ms = t_start.elapsed().as_secs_f64() * 1000.0;
        total_times.push(total_ms);

        if i < 5 || i % 10 == 0 {
            println!(
                "Iteration {}: Cap Latency = {:.3} ms (fetch: {:.3} ms) | Infer = {:.2} ms | Total = {:.2} ms ({:.0} FPS)",
                i, cap_ms, cap_fetch_time, infer_ms, total_ms, 1000.0 / total_ms
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    capture_control.stop().unwrap();

    let avg_cap = cap_times.iter().sum::<f64>() / cap_times.len() as f64;
    let avg_infer = infer_times.iter().sum::<f64>() / infer_times.len() as f64;
    let avg_total = total_times.iter().sum::<f64>() / total_times.len() as f64;

    println!("\n==========================================");
    println!(">>> WGC DIRECT GPU STREAM BENCHMARK <<<");
    println!(">>> Average Capture Latency:   {:.3} ms", avg_cap);
    println!(">>> Average Inference Latency: {:.3} ms", avg_infer);
    println!(">>> Average Total Frame Time:  {:.3} ms", avg_total);
    println!(">>> PIPELINE SPEED:            {:.1} FPS", 1000.0 / avg_total);
    println!("==========================================\n");
}

