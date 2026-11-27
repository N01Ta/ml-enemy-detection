#[test]
fn test_affinity_impact() {
    use std::time::Instant;
    use std::ffi::c_void;

    type HWND = *mut c_void;
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
        fn GetDC(hWnd: HWND) -> HDC;
        fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
        fn GetSystemMetrics(nIndex: i32) -> i32;
        fn FindWindowA(lpClassName: *const i8, lpWindowName: *const i8) -> HWND;
        fn SetWindowDisplayAffinity(hWnd: HWND, dwAffinity: u32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: HDC) -> HDC;
        fn CreateDIBSection(hdc: HDC, pbmi: *const BITMAPINFO, usage: u32, ppvBits: *mut *mut c_void, hSection: *mut c_void, offset: u32) -> HBITMAP;
        fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
        fn StretchBlt(hdcDest: HDC, xDest: i32, yDest: i32, wDest: i32, hDest: i32, hdcSrc: HDC, xSrc: i32, ySrc: i32, wSrc: i32, hSrc: i32, rop: DWORD) -> BOOL;
        fn SetStretchBltMode(hdc: HDC, mode: i32) -> i32;
        fn DeleteObject(ho: HGDIOBJ) -> BOOL;
        fn DeleteDC(hdc: HDC) -> BOOL;
    }

    unsafe {
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
        let h_dib_bitmap = CreateDIBSection(
            h_mem_dc,
            &bmi32,
            DIB_RGB_COLORS,
            &mut dib_bits,
            std::ptr::null_mut(),
            0,
        );
        SelectObject(h_mem_dc, h_dib_bitmap);

        // Benchmark without overlay
        let iters = 20;
        let start = Instant::now();
        for _ in 0..iters {
            StretchBlt(h_mem_dc, 0, 0, 640, 640, h_screen_dc, left, top, fov, fov, SRCCOPY);
        }
        let avg_plain = (start.elapsed().as_secs_f64() * 1000.0) / (iters as f64);
        println!("StretchBlt without overlay: {:.3} ms", avg_plain);

        DeleteObject(h_dib_bitmap);
        DeleteDC(h_mem_dc);
        ReleaseDC(std::ptr::null_mut(), h_screen_dc);
    }
}
