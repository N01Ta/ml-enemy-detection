#[test]
fn test_stretch_blt_speed() {
    use std::time::Instant;
    use std::ffi::c_void;

    type HWND = *mut c_void;
    type HDC = *mut c_void;
    type HBITMAP = *mut c_void;
    type HGDIOBJ = *mut c_void;
    type BOOL = i32;
    type DWORD = u32;
    type LONG = i32;
    type WORD = u16;

    const SRCCOPY: DWORD = 0x00CC0020;
    const DIB_RGB_COLORS: u32 = 0;
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    const COLORONCOLOR: i32 = 3;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct BITMAPINFOHEADER {
        biSize: DWORD,
        biWidth: LONG,
        biHeight: LONG,
        biPlanes: WORD,
        biBitCount: WORD,
        biCompression: DWORD,
        biSizeImage: DWORD,
        biXPelsPerMeter: LONG,
        biYPelsPerMeter: LONG,
        biClrUsed: DWORD,
        biClrImportant: DWORD,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RGBQUAD {
        rgbBlue: u8,
        rgbGreen: u8,
        rgbRed: u8,
        rgbReserved: u8,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER,
        bmiColors: [RGBQUAD; 1],
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hWnd: HWND) -> HDC;
        fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: HDC) -> HDC;
        fn CreateDIBSection(hdc: HDC, pbmi: *const BITMAPINFO, usage: u32, ppvBits: *mut *mut c_void, hSection: *mut c_void, offset: u32) -> HBITMAP;
        fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
        fn BitBlt(hdc: HDC, x: i32, y: i32, cx: i32, cy: i32, hdcSrc: HDC, x1: i32, y1: i32, rop: DWORD) -> BOOL;
        fn StretchBlt(hdcDest: HDC, xDest: i32, yDest: i32, wDest: i32, hDest: i32, hdcSrc: HDC, xSrc: i32, ySrc: i32, wSrc: i32, hSrc: i32, rop: DWORD) -> BOOL;
        fn SetStretchBltMode(hdc: HDC, mode: i32) -> i32;
        fn DeleteObject(ho: HGDIOBJ) -> BOOL;
        fn DeleteDC(hdc: HDC) -> BOOL;
    }

    let dst_w = 640;
    let dst_h = 640;

    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        let h_screen_dc = GetDC(std::ptr::null_mut());
        let h_mem_dc = CreateCompatibleDC(h_screen_dc);
        SetStretchBltMode(h_mem_dc, COLORONCOLOR);

        let mut bmi32 = std::mem::zeroed::<BITMAPINFO>();
        bmi32.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi32.bmiHeader.biWidth = dst_w;
        bmi32.bmiHeader.biHeight = -dst_h;
        bmi32.bmiHeader.biPlanes = 1;
        bmi32.bmiHeader.biBitCount = 32;
        bmi32.bmiHeader.biCompression = 0;

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

        for fov in [320, 640, 1000, 1400] {
            let left = (screen_w - fov) / 2;
            let top = (screen_h - fov) / 2;

            let iters = 30;
            let start = Instant::now();
            let mut rgb_buf = vec![0u8; (dst_w * dst_h * 3) as usize];

            for _ in 0..iters {
                StretchBlt(
                    h_mem_dc,
                    0,
                    0,
                    dst_w,
                    dst_h,
                    h_screen_dc,
                    left,
                    top,
                    fov,
                    fov,
                    SRCCOPY,
                );

                let mut src = dib_bits as *const u8;
                let mut dst = rgb_buf.as_mut_ptr();
                let end = src.add((dst_w * dst_h * 4) as usize);
                while src < end {
                    *dst = *src.add(2);
                    *dst.add(1) = *src.add(1);
                    *dst.add(2) = *src;
                    src = src.add(4);
                    dst = dst.add(3);
                }
            }

            let avg_ms = (start.elapsed().as_secs_f64() * 1000.0) / (iters as f64);
            println!("FOV {} -> 640x640: avg total capture = {:.3} ms ({:.1} FPS)", fov, avg_ms, 1000.0 / avg_ms);
        }

        DeleteObject(h_dib_bitmap);
        DeleteDC(h_mem_dc);
        ReleaseDC(std::ptr::null_mut(), h_screen_dc);
    }
}
