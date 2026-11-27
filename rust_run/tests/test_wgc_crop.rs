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

struct CropHandler {
    frame_count: u32,
    times: Vec<f64>,
    rgb_buf: Vec<u8>,
}

impl GraphicsCaptureApiHandler for CropHandler {
    type Flags = ();
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(_ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame_count: 0,
            times: Vec::new(),
            rgb_buf: vec![0u8; 640 * 640 * 3],
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        self.frame_count += 1;
        let w = frame.width();
        let h = frame.height();

        let crop_w = 640;
        let crop_h = 640;
        let start_x = (w - crop_w) / 2;
        let start_y = (h - crop_h) / 2;
        let end_x = start_x + crop_w;
        let end_y = start_y + crop_h;

        let t0 = Instant::now();
        let mut crop_buf = frame.buffer_crop(start_x, start_y, end_x, end_y)?;
        let row_pitch = crop_buf.row_pitch() as usize;
        let raw = crop_buf.as_raw_buffer();

        // Convert RGBA8 to RGB24
        let dst_ptr = self.rgb_buf.as_mut_ptr();
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
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        self.times.push(elapsed);

        if self.frame_count <= 5 || self.frame_count % 10 == 0 {
            println!(
                "Frame {}: GPU Crop 640x640 + Map + RGB convert = {:.3} ms",
                self.frame_count, elapsed
            );
        }

        if self.frame_count >= 25 {
            capture_control.stop();
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        if !self.times.is_empty() {
            let avg = self.times.iter().sum::<f64>() / self.times.len() as f64;
            println!(
                "\n>>> WGC GPU CROP AVERAGE LATENCY: {:.3} ms ({:.1} FPS) <<<\n",
                avg,
                1000.0 / avg
            );
        }
        Ok(())
    }
}

#[test]
fn test_wgc_gpu_crop_speed() {
    let monitor = Monitor::primary().unwrap();
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (),
    );

    println!("Testing WGC GPU-cropped 640x640 capture speed...");
    CropHandler::start(settings).unwrap();
}
