use std::time::{Duration, Instant};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use procmod_overlay::{Color, Overlay, OverlayTarget};

type HDC = *mut c_void;
type HBITMAP = *mut c_void;
type HGDIOBJ = *mut c_void;
type BOOL = i32;
type DWORD = u32;

const SRCCOPY: DWORD = 0x00CC0020;
const DIB_RGB_COLORS: u32 = 0;
const COLORONCOLOR: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFOHEADER {
    bi_size: DWORD,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: DWORD,
    bi_size_image: DWORD,
    bi_xpels_per_meter: i32,
    bi_ypels_per_meter: i32,
    bi_clr_used: DWORD,
    bi_clr_important: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RGBQUAD {
    rgb_blue: u8,
    rgb_green: u8,
    rgb_red: u8,
    rgb_reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [RGBQUAD; 1],
}

#[link(name = "user32")]
extern "system" {
    fn GetDesktopWindow() -> *mut c_void;
    fn GetDC(hWnd: *mut c_void) -> HDC;
    fn ReleaseDC(hWnd: *mut c_void, hDC: HDC) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: u32,
        ppvBits: *mut *mut c_void,
        hSection: *mut c_void,
        offset: u32,
    ) -> HBITMAP;
    fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    fn StretchBlt(
        hdcDest: HDC,
        xDest: i32,
        yDest: i32,
        wDest: i32,
        hDest: i32,
        hdcSrc: HDC,
        xSrc: i32,
        ySrc: i32,
        wSrc: i32,
        hSrc: i32,
        rop: DWORD,
    ) -> BOOL;
    fn SetStretchBltMode(hdc: HDC, mode: i32) -> i32;
    fn GdiSetBatchLimit(dwLimit: DWORD) -> DWORD;
    fn GdiFlush() -> BOOL;
    fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    fn DeleteDC(hdc: HDC) -> BOOL;
}

#[test]
fn test_capture_with_overlay_active() {
    let desktop_hwnd = unsafe { GetDesktopWindow() } as isize;
    let is_running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&is_running);

    // Overlay running at maximum unthrottled Present(0) rate
    let overlay_thread = std::thread::spawn(move || {
        let mut overlay = Overlay::new(OverlayTarget::Hwnd(desktop_hwnd)).unwrap();
        while running_clone.load(Ordering::Relaxed) {
            if overlay.begin_frame().is_ok() {
                overlay.circle(500.0, 500.0, 150.0, Color::CYAN);
                overlay.rect(300.0, 300.0, 100.0, 200.0, Color::RED);
                overlay.text(300.0, 280.0, "Apex Test [98%]", 14.0, Color::WHITE);
                let _ = overlay.end_frame();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    unsafe {
        GdiSetBatchLimit(1);

        let screen_w = GetSystemMetrics(0);
        let screen_h = GetSystemMetrics(1);
        let fov = 640;
        let left = (screen_w - fov) / 2;
        let top = (screen_h - fov) / 2;

        let h_screen_dc = GetDC(std::ptr::null_mut());
        let h_mem_dc = CreateCompatibleDC(h_screen_dc);
        SetStretchBltMode(h_mem_dc, COLORONCOLOR);

        let mut bmi32 = std::mem::zeroed::<BITMAPINFO>();
        bmi32.bmi_header.bi_size = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi32.bmi_header.bi_width = 640;
        bmi32.bmi_header.bi_height = -640;
        bmi32.bmi_header.bi_planes = 1;
        bmi32.bmi_header.bi_bit_count = 32;

        let mut dib_bits: *mut c_void = std::ptr::null_mut();
        let h_dib = CreateDIBSection(h_mem_dc, &bmi32, DIB_RGB_COLORS, &mut dib_bits, std::ptr::null_mut(), 0);
        SelectObject(h_mem_dc, h_dib);

        let mut rgb_buf = vec![0u8; 640 * 640 * 3];

        let iters = 50;
        let mut times = Vec::new();

        for i in 0..iters {
            let t0 = Instant::now();
            StretchBlt(h_mem_dc, 0, 0, 640, 640, h_screen_dc, left, top, fov, fov, SRCCOPY);
            GdiFlush();
            let mut src = dib_bits as *const u8;
            let mut dst = rgb_buf.as_mut_ptr();
            let end = src.add(640 * 640 * 4);
            while src < end {
                *dst = *src.add(2);
                *dst.add(1) = *src.add(1);
                *dst.add(2) = *src;
                src = src.add(4);
                dst = dst.add(3);
            }
            let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
            times.push(elapsed);
            if i < 5 || i % 10 == 0 {
                println!("Frame {}: {:.3} ms", i, elapsed);
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        is_running.store(false, Ordering::Relaxed);
        let _ = overlay_thread.join();

        let avg = times.iter().sum::<f64>() / times.len() as f64;
        println!("Average capture latency WITH overlay active: {:.3} ms ({:.1} FPS)", avg, 1000.0 / avg);

        DeleteObject(h_dib);
        DeleteDC(h_mem_dc);
        ReleaseDC(std::ptr::null_mut(), h_screen_dc);
    }
}
