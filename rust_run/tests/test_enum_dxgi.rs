use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

#[test]
fn test_enum() {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().unwrap();
        for i in 0.. {
            let adapter = match factory.EnumAdapters1(i) {
                Ok(a) => a,
                Err(_) => break,
            };
            let desc = match adapter.GetDesc1() {
                Ok(d) => d,
                Err(_) => break,
            };
            let name = String::from_utf16_lossy(&desc.Description);
            let name = name.trim_matches(char::from(0));
            println!("Adapter {}: {} (VendorId: {:x})", i, name, desc.VendorId);

            for j in 0.. {
                let output = match adapter.EnumOutputs(j) {
                    Ok(o) => o,
                    Err(_) => break,
                };
                let out_desc = match output.GetDesc() {
                    Ok(d) => d,
                    Err(_) => break,
                };
                let out_name = String::from_utf16_lossy(&out_desc.DeviceName);
                let out_name = out_name.trim_matches(char::from(0));
                let r = out_desc.DesktopCoordinates;
                println!("  Output {}: {} ({},{} - {},{}) attached={}", j, out_name, r.left, r.top, r.right, r.bottom, out_desc.AttachedToDesktop.0);
            }
        }
    }
}
