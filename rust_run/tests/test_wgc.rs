use std::time::Instant;
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

struct CaptureTestHandler {
    frame_count: u32,
    last_frame_time: Instant,
    total_latency_ms: f64,
}

impl GraphicsCaptureApiHandler for CaptureTestHandler {
    type Flags = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(_ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame_count: 0,
            last_frame_time: Instant::now(),
            total_latency_ms: 0.0,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let t0 = Instant::now();
        self.frame_count += 1;
        let delta = self.last_frame_time.elapsed().as_secs_f64() * 1000.0;
        self.last_frame_time = Instant::now();

        // Read or crop buffer
        let width = frame.width();
        let height = frame.height();
        
        let crop_t0 = Instant::now();
        // Test frame buffer copy / crop
        let mut buffer = frame.buffer()?;
        let crop_elapsed = crop_t0.elapsed().as_secs_f64() * 1000.0;

        self.total_latency_ms += crop_elapsed;

        if self.frame_count <= 10 || self.frame_count % 10 == 0 {
            println!(
                "WGC Frame {}: {}x{} | buffer access: {:.3} ms | frame delta: {:.2} ms ({:.1} FPS)",
                self.frame_count,
                width,
                height,
                crop_elapsed,
                delta,
                1000.0 / delta
            );
        }

        if self.frame_count >= 30 {
            capture_control.stop();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        println!("Capture session closed cleanly!");
        Ok(())
    }
}

#[test]
fn test_wgc_capture() {
    let primary_monitor = Monitor::primary().expect("No primary monitor found");
    println!("Primary monitor: {}", primary_monitor.name().unwrap_or_default());

    let settings = Settings::new(
        primary_monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (),
    );

    println!("Starting Windows.Graphics.Capture session...");
    CaptureTestHandler::start(settings).expect("Failed to start WGC session");
    println!("WGC session completed successfully!");
}
