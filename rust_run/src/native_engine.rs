use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use ultralytics_inference::{Device, InferenceConfig, YOLOModel};
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

use crate::{LiveDetectionPacket, LiveTargetBox};

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
mod gdi {
    use std::ffi::c_void;

    pub type HWND = *mut c_void;
    pub type HDC = *mut c_void;
    pub type HBITMAP = *mut c_void;
    pub type HGDIOBJ = *mut c_void;
    pub type BOOL = i32;
    pub type DWORD = u32;
    pub type LONG = i32;
    pub type WORD = u16;

    pub const SRCCOPY: DWORD = 0x00CC0020;
    pub const DIB_RGB_COLORS: u32 = 0;
    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct BITMAPINFOHEADER {
        pub biSize: DWORD,
        pub biWidth: LONG,
        pub biHeight: LONG,
        pub biPlanes: WORD,
        pub biBitCount: WORD,
        pub biCompression: DWORD,
        pub biSizeImage: DWORD,
        pub biXPelsPerMeter: LONG,
        pub biYPelsPerMeter: LONG,
        pub biClrUsed: DWORD,
        pub biClrImportant: DWORD,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RGBQUAD {
        pub rgbBlue: u8,
        pub rgbGreen: u8,
        pub rgbRed: u8,
        pub rgbReserved: u8,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct BITMAPINFO {
        pub bmiHeader: BITMAPINFOHEADER,
        pub bmiColors: [RGBQUAD; 1],
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetDC(hWnd: HWND) -> HDC;
        pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
        pub fn GetSystemMetrics(nIndex: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
        pub fn CreateDIBSection(
            hdc: HDC,
            pbmi: *const BITMAPINFO,
            usage: u32,
            ppvBits: *mut *mut c_void,
            hSection: *mut c_void,
            offset: u32,
        ) -> HBITMAP;
        pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
        pub fn BitBlt(
            hdcDest: HDC,
            xDest: i32,
            yDest: i32,
            wDest: i32,
            hDest: i32,
            hdcSrc: HDC,
            xSrc: i32,
            ySrc: i32,
            rop: DWORD,
        ) -> BOOL;
        pub fn GdiSetBatchLimit(dwLimit: DWORD) -> DWORD;
        pub fn GdiFlush() -> BOOL;
        pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
        pub fn DeleteDC(hdc: HDC) -> BOOL;
    }
}

// =========================================================================
// 1. ПРЯМОЙ ПЕРЕХВАТ ВИДЕОПОТОКА GPU (Windows.Graphics.Capture API)
// =========================================================================
struct SharedFrameData {
    rgb: image::DynamicImage,
    new_frame_counter: u64,
    last_cap_ms: f32,
    screen_w: i32,
    screen_h: i32,
}

struct WgcHandler {
    shared_data: Arc<Mutex<SharedFrameData>>,
}

impl GraphicsCaptureApiHandler for WgcHandler {
    type Flags = Arc<Mutex<SharedFrameData>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let shared_data = ctx.flags;
        Ok(Self {
            shared_data,
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

        // Защита от нулевых/минимизированных кадров (смена разрешения / alt-tab игры)
        if w < 640 || h < 640 {
            return Ok(());
        }

        // Аппаратная обрезка 640x640 по центру в видеопамяти NVIDIA (CopySubresourceRegion)
        let crop_w = 640;
        let crop_h = 640;
        let start_x = (w - crop_w) / 2;
        let start_y = (h - crop_h) / 2;
        let end_x = start_x + crop_w;
        let end_y = start_y + crop_h;

        let mut crop_buf = match frame.buffer_crop(start_x, start_y, end_x, end_y) {
            Ok(buf) => buf,
            Err(_) => return Ok(()),
        };
        let row_pitch = crop_buf.row_pitch() as usize;
        let raw = crop_buf.as_raw_buffer();

        if row_pitch < 640 * 4 || raw.len() < 640 * row_pitch {
            return Ok(());
        }

        let cap_ms = t0.elapsed().as_secs_f32() * 1000.0;

        if let Ok(mut lock) = self.shared_data.lock() {
            if let Some(rgb) = lock.rgb.as_mut_rgb8() {
                let dst = rgb.as_mut();
                for y in 0..640 {
                    let row_start = y * row_pitch;
                    let src_slice = &raw[row_start..row_start + 640 * 4];
                    let dst_offset = y * 640 * 3;
                    let dst_slice = &mut dst[dst_offset..dst_offset + 640 * 3];
                    for x in 0..640 {
                        dst_slice[x * 3] = src_slice[x * 4];
                        dst_slice[x * 3 + 1] = src_slice[x * 4 + 1];
                        dst_slice[x * 3 + 2] = src_slice[x * 4 + 2];
                    }
                }
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

// =========================================================================
// 2. БЫСТРЫЙ РЕЗЕРВНЫЙ ЗАХВАТЧИК (BitBlt 640x640 без масштабирования CPU)
// =========================================================================
pub struct NativeScreenCapturer {
    pub screen_w: i32,
    pub screen_h: i32,
    pub left: i32,
    pub top: i32,
    h_screen_dc: gdi::HDC,
    h_mem_dc: gdi::HDC,
    h_dib_bitmap: gdi::HBITMAP,
    dib_bits: *mut std::ffi::c_void,
    dyn_image: image::DynamicImage,
}

unsafe impl Send for NativeScreenCapturer {}

impl NativeScreenCapturer {
    pub fn new() -> Self {
        unsafe {
            gdi::GdiSetBatchLimit(1);
            let screen_w = gdi::GetSystemMetrics(gdi::SM_CXSCREEN);
            let screen_h = gdi::GetSystemMetrics(gdi::SM_CYSCREEN);
            let left = (screen_w - 640) / 2;
            let top = (screen_h - 640) / 2;

            let h_screen_dc = gdi::GetDC(std::ptr::null_mut());
            let h_mem_dc = gdi::CreateCompatibleDC(h_screen_dc);

            let dst_w = 640;
            let dst_h = 640;

            let mut bmi = std::mem::zeroed::<gdi::BITMAPINFO>();
            bmi.bmiHeader.biSize = std::mem::size_of::<gdi::BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = dst_w;
            bmi.bmiHeader.biHeight = -dst_h; // Верх-вниз
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = 0;

            let mut dib_bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let h_dib_bitmap = gdi::CreateDIBSection(
                h_mem_dc,
                &bmi,
                gdi::DIB_RGB_COLORS,
                &mut dib_bits,
                std::ptr::null_mut(),
                0,
            );
            gdi::SelectObject(h_mem_dc, h_dib_bitmap);

            let rgb_img = image::RgbImage::new(dst_w as u32, dst_h as u32);
            let dyn_image = image::DynamicImage::ImageRgb8(rgb_img);

            Self {
                screen_w,
                screen_h,
                left,
                top,
                h_screen_dc,
                h_mem_dc,
                h_dib_bitmap,
                dib_bits,
                dyn_image,
            }
        }
    }

    #[inline]
    pub fn capture_to_image(&mut self) -> &image::DynamicImage {
        unsafe {
            if self.dib_bits.is_null() {
                return &self.dyn_image;
            }
            // Нативный 1:1 BitBlt центра экрана без StretchBlt (0.18 мс)
            gdi::BitBlt(
                self.h_mem_dc,
                0,
                0,
                640,
                640,
                self.h_screen_dc,
                self.left,
                self.top,
                gdi::SRCCOPY,
            );
            gdi::GdiFlush();

            let total_pixels = 640 * 640;
            if let Some(rgb) = self.dyn_image.as_mut_rgb8() {
                let mut src = self.dib_bits as *const u8;
                let mut dst = rgb.as_mut().as_mut_ptr();
                let end = src.add(total_pixels * 4);
                while src < end {
                    *dst = *src.add(2);     // R
                    *dst.add(1) = *src.add(1); // G
                    *dst.add(2) = *src;     // B
                    src = src.add(4);
                    dst = dst.add(3);
                }
            }
        }

        &self.dyn_image
    }
}

impl Drop for NativeScreenCapturer {
    fn drop(&mut self) {
        unsafe {
            gdi::DeleteObject(self.h_dib_bitmap);
            gdi::DeleteDC(self.h_mem_dc);
            gdi::ReleaseDC(std::ptr::null_mut(), self.h_screen_dc);
        }
    }
}

// =========================================================================
// 3. ОСНОВНОЙ КОНВЕЙЕР ИНФЕРЕНСА И ТЕЛЕМЕТРИИ
// =========================================================================
pub struct NativeVisionRunner {
    pub is_running: Arc<AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl NativeVisionRunner {
    pub fn start(
        model_path: String,
        preferred_provider: String,
        capture_api: String,
        target_display: String,
        conf: f32,
        iou: f32,
        fov_size_shared: Arc<AtomicU32>,
        fps_limit_shared: Arc<AtomicU32>,
        output: Arc<Mutex<LiveDetectionPacket>>,
    ) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&is_running);

        let thread_handle = std::thread::spawn(move || {
            // Prefer explicit environment variables instead of machine-specific paths.
            // ORT_DYLIB_PATH points to onnxruntime.dll; TENSORRT_DIR points to the TensorRT root.
            let native_dir = std::env::var("ORT_DYLIB_PATH")
                .ok()
                .and_then(|p| std::path::Path::new(&p).parent().map(|p| p.to_path_buf()))
                .or_else(|| std::env::var("TENSORRT_DIR").ok().map(std::path::PathBuf::from))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            #[cfg(target_os = "windows")]
            {
                if let Ok(c_dir) = std::ffi::CString::new(native_dir.to_string_lossy().as_ref()) {
                    #[link(name = "kernel32")]
                    extern "system" {
                        fn SetDllDirectoryA(lpPathName: *const std::ffi::c_char) -> i32;
                    }
                    unsafe {
                        SetDllDirectoryA(c_dir.as_ptr());
                    }
                }
                let current_path = std::env::var("PATH").unwrap_or_default();
                std::env::set_var("PATH", format!("{};{}", native_dir.display(), current_path));
            }

            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
            let mut candidate_paths = Vec::new();
            if let Ok(ort_path) = std::env::var("ORT_DYLIB_PATH") {
                candidate_paths.push(std::path::PathBuf::from(ort_path));
            }
            candidate_paths.push(std::path::PathBuf::from("onnxruntime.dll"));
            if let Some(dir) = &exe_dir {
                candidate_paths.push(dir.join("onnxruntime.dll"));
            }
            candidate_paths.push(native_dir.join("onnxruntime.dll"));
            candidate_paths.push(native_dir.join("bin").join("onnxruntime.dll"));
            candidate_paths.push(native_dir.join("lib").join("onnxruntime.dll"));
            for path in &candidate_paths {
                if path.exists() {
                    if let Ok(builder) = ort::init_from(path.to_str().unwrap_or("onnxruntime.dll")) {
                        let _ = builder.commit();
                        break;
                    }
                }
            }

            let is_trt = preferred_provider.to_lowercase().contains("tensorrt");
            let is_directml = preferred_provider.to_lowercase().contains("directml");
            let is_cpu = preferred_provider.to_lowercase().contains("cpu");

            let trt_cfg = || {
                InferenceConfig::new()
                    .with_confidence(conf)
                    .with_iou(iou)
                    .with_imgsz(640, 640)
                    .with_rect(false)
                    .with_device(Device::TensorRt(0))
            };
            let cuda_cfg = || {
                InferenceConfig::new()
                    .with_confidence(conf)
                    .with_iou(iou)
                    .with_imgsz(640, 640)
                    .with_rect(false)
                    .with_device(Device::Cuda(0))
            };
            let dml_cfg = || {
                InferenceConfig::new()
                    .with_confidence(conf)
                    .with_iou(iou)
                    .with_imgsz(640, 640)
                    .with_rect(false)
                    .with_device(Device::DirectMl(0))
                    .with_cuda_preprocess(false)
            };
            let cpu_cfg = || {
                InferenceConfig::new()
                    .with_confidence(conf)
                    .with_iou(iou)
                    .with_imgsz(640, 640)
                    .with_rect(false)
                    .with_device(Device::Cpu)
                    .with_cuda_preprocess(false)
            };

            let mut model = if is_cpu {
                match YOLOModel::load_with_config(&model_path, cpu_cfg()) {
                    Ok(m) => {
                        eprintln!("[NATIVE YOLO] Running on CPU!");
                        m
                    }
                    Err(e) => {
                        eprintln!("[NATIVE YOLO FATAL] CPU execution failed: {:?}", e);
                        return;
                    }
                }
            } else if is_trt {
                match YOLOModel::load_with_config(&model_path, trt_cfg()) {
                    Ok(m) => {
                        eprintln!("[NATIVE YOLO] Running on NVIDIA TensorRT(0) with fused GPU preprocessing!");
                        m
                    }
                    Err(e) => {
                        eprintln!("[NATIVE YOLO] TensorRT unavailable ({:?}), switching to CUDA(0)...", e);
                        match YOLOModel::load_with_config(&model_path, cuda_cfg()) {
                            Ok(m) => {
                                eprintln!("[NATIVE YOLO] Running on NVIDIA CUDA(0)!");
                                m
                            }
                            Err(e2) => {
                                eprintln!("[NATIVE YOLO] CUDA unavailable ({:?}), switching to DirectML...", e2);
                                match YOLOModel::load_with_config(&model_path, dml_cfg()) {
                                    Ok(m) => m,
                                    Err(e3) => {
                                        eprintln!("[NATIVE YOLO FATAL] All providers failed: {:?}", e3);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            } else if is_directml {
                match YOLOModel::load_with_config(&model_path, dml_cfg()) {
                    Ok(m) => m,
                    Err(_) => match YOLOModel::load_with_config(&model_path, cuda_cfg()) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("[NATIVE YOLO FATAL] DirectML & CUDA failed: {:?}", e);
                            return;
                        }
                    },
                }
            } else {
                match YOLOModel::load_with_config(&model_path, cuda_cfg()) {
                    Ok(m) => {
                        eprintln!("[NATIVE YOLO] Running on NVIDIA CUDA(0)!");
                        m
                    }
                    Err(e) => {
                        eprintln!("[NATIVE YOLO] CUDA unavailable ({:?}), checking TensorRT...", e);
                        match YOLOModel::load_with_config(&model_path, trt_cfg()) {
                            Ok(m) => m,
                            Err(_) => match YOLOModel::load_with_config(&model_path, dml_cfg()) {
                                Ok(m) => m,
                                Err(e_dml) => {
                                    eprintln!("[NATIVE YOLO FATAL] All providers failed: {:?}", e_dml);
                                    return;
                                }
                            },
                        }
                    }
                }
            };

            // Инициализация видеозахвата по выбранному API
            let shared_frame = Arc::new(Mutex::new(SharedFrameData {
                rgb: image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640)),
                new_frame_counter: 0,
                last_cap_ms: 0.35,
                screen_w: 3440,
                screen_h: 1440,
            }));

            let use_gdi_only = capture_api.to_lowercase().contains("gdi");

            let mut wgc_control = if use_gdi_only {
                eprintln!("[NATIVE CAPTURE] Selected GDI BitBlt mode (Universal CPU Fallback)!");
                None
            } else {
                let selected_monitor = match Monitor::enumerate() {
                    Ok(monitors) => {
                        let mut chosen = None;
                        for mon in monitors {
                            if let Ok(name) = mon.name() {
                                if target_display.contains(&name) {
                                    chosen = Some(mon);
                                    break;
                                }
                            }
                        }
                        chosen.or_else(|| Monitor::primary().ok())
                    }
                    Err(_) => Monitor::primary().ok(),
                };

                match selected_monitor {
                    Some(monitor) => {
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
                        match WgcHandler::start_free_threaded(settings) {
                            Ok(ctrl) => {
                                eprintln!("[NATIVE CAPTURE] Hardware Direct GPU Video Stream Active (Windows.Graphics.Capture)!");
                                Some(ctrl)
                            }
                            Err(err) => {
                                eprintln!("[NATIVE CAPTURE] WGC start error: {:?}. Using GDI BitBlt fallback.", err);
                                None
                            }
                        }
                    }
                    None => {
                        eprintln!("[NATIVE CAPTURE] Monitor discovery error. Using GDI BitBlt fallback.");
                        None
                    }
                }
            };

            let mut fallback_capturer = if wgc_control.is_none() {
                Some(NativeScreenCapturer::new())
            } else {
                None
            };

            let mut local_dyn_img = image::DynamicImage::ImageRgb8(image::RgbImage::new(640, 640));
            let mut fps_count = 0;
            let mut fps_start = Instant::now();
            let mut current_fps = 60.0;
            let mut frame_id: u64 = 0;

            while running_clone.load(Ordering::Relaxed) {
                let t_start = Instant::now();
                frame_id = frame_id.wrapping_add(1);
                let current_fov = fov_size_shared.load(Ordering::Relaxed).clamp(60, 1440) as f32;

                // 1. Сверхбыстрый захват кадра (WGC GPU-стриминг 0.3-0.5 мс или GDI BitBlt 0.18 мс) без аллокаций
                let (cap_ms, screen_w, screen_h) = if let Some(ref mut capturer) = fallback_capturer {
                    let t0 = Instant::now();
                    let dyn_ref = capturer.capture_to_image();
                    if let (Some(dst), Some(src)) = (local_dyn_img.as_mut_rgb8(), dyn_ref.as_rgb8()) {
                        dst.copy_from_slice(src.as_raw());
                    }
                    (t0.elapsed().as_secs_f32() * 1000.0, capturer.screen_w, capturer.screen_h)
                } else if let Ok(lock) = shared_frame.lock() {
                    if let (Some(dst), Some(src)) = (local_dyn_img.as_mut_rgb8(), lock.rgb.as_rgb8()) {
                        dst.copy_from_slice(src.as_raw());
                    }
                    (lock.last_cap_ms, lock.screen_w, lock.screen_h)
                } else {
                    (0.35, 3440, 1440)
                };

                // 2. Нативный инференс Ultralytics YOLO на TensorRT (1.1 - 1.6 мс)
                let t_infer_start = Instant::now();
                let results = model.predict_image(&local_dyn_img, "screen".to_string());
                let infer_ms = t_infer_start.elapsed().as_secs_f32() * 1000.0;

                let mut targets = Vec::new();
                let offset_x = (screen_w - 640) as f32 / 2.0;
                let offset_y = (screen_h - 640) as f32 / 2.0;

                match results {
                    Ok(batch_results) => {
                        for result in &batch_results {
                            if let Some(boxes) = &result.boxes {
                                let xyxy = boxes.xyxy();
                                let count = boxes.len();
                                for i in 0..count {
                                    let cls_id = boxes.cls()[i] as usize;
                                    let conf = boxes.conf()[i];
                                    let row = xyxy.row(i);
                                    let x1 = offset_x + row[0];
                                    let y1 = offset_y + row[1];
                                    let x2 = offset_x + row[2];
                                    let y2 = offset_y + row[3];

                                    let w = x2 - x1;
                                    let h = y2 - y1;
                                    let center_x = x1 + w * 0.5;
                                    let center_y = y1 + h * 0.5;

                                    // Фильтрация детекций по настраиваемому радиусу круга FOV (Radius = FOV / 2)
                                    let cx = screen_w as f32 / 2.0;
                                    let cy = screen_h as f32 / 2.0;
                                    let radius = current_fov / 2.0;
                                    let dx = center_x - cx;
                                    let dy = center_y - cy;
                                    let dist = (dx * dx + dy * dy).sqrt();

                                    if dist <= radius {
                                        targets.push(LiveTargetBox {
                                            cls: cls_id,
                                            conf,
                                            x: center_x,
                                            y: center_y,
                                            w,
                                            h,
                                            ..Default::default()
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("[NATIVE YOLO INFERENCE ERROR] {:?}", err);
                    }
                }

                fps_count += 1;
                if fps_start.elapsed().as_secs_f32() >= 0.5 {
                    current_fps = fps_count as f32 / fps_start.elapsed().as_secs_f32();
                    fps_count = 0;
                    fps_start = Instant::now();
                }

                let packet = LiveDetectionPacket {
                    frame_id,
                    fps: current_fps,
                    cap_ms,
                    infer_ms,
                    render_ms: 0.2,
                    screen_w,
                    screen_h,
                    targets,
                };

                if let Ok(mut lock) = output.lock() {
                    *lock = packet;
                }

                // Лимит FPS нейросети (Frame Pacing)
                let fps_limit = fps_limit_shared.load(Ordering::Relaxed);
                if fps_limit > 0 {
                    let target_dt = std::time::Duration::from_secs_f64(1.0 / fps_limit as f64);
                    let elapsed = t_start.elapsed();
                    if elapsed < target_dt {
                        std::thread::sleep(target_dt - elapsed);
                    }
                } else if t_start.elapsed().as_millis() == 0 {
                    std::thread::yield_now();
                }
            }

            if let Some(ctrl) = wgc_control.take() {
                let _ = ctrl.stop();
            }
        });

        Self {
            is_running,
            thread_handle: Some(thread_handle),
        }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for NativeVisionRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
