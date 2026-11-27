use procmod_overlay::{Color, Overlay, OverlayTarget};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

#[path = "../src/native_engine.rs"]
mod native_engine;
use native_engine::NativeScreenCapturer;

#[link(name = "user32")]
extern "system" {
    fn GetDesktopWindow() -> *mut std::ffi::c_void;
}

#[test]
fn test_capture_while_overlay_active() {
    let desktop_hwnd = unsafe { GetDesktopWindow() };
    let is_running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&is_running);

    let desktop_hwnd_val = desktop_hwnd as isize;
    let overlay_thread = std::thread::spawn(move || {
        let mut overlay = match Overlay::new(OverlayTarget::Hwnd(desktop_hwnd_val)) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to create overlay: {:?}", e);
                return;
            }
        };

        while running_clone.load(Ordering::Relaxed) {
            if overlay.begin_frame().is_ok() {
                overlay.rect(500.0, 500.0, 200.0, 300.0, Color::RED);
                overlay.text(500.0, 480.0, "Apex Enemy [95%]", 14.0, Color::WHITE);
                let _ = overlay.end_frame();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    let mut capturer = NativeScreenCapturer::new();
    println!("Capturer initialized. Starting 20 capture iterations while overlay is running...");

    let mut times = Vec::new();
    for i in 0..20 {
        let t0 = Instant::now();
        let _ = capturer.capture_to_image();
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        times.push(elapsed);
        println!("Iteration {}: {:.3} ms", i, elapsed);
        std::thread::sleep(Duration::from_millis(5));
    }

    is_running.store(false, Ordering::Relaxed);
    let _ = overlay_thread.join();

    let avg = times.iter().sum::<f64>() / times.len() as f64;
    println!("Average capture latency while overlay running: {:.3} ms", avg);
}
