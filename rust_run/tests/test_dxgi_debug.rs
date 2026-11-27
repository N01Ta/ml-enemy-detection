use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

#[test]
fn test_dxgi_duplication_correct() {
    unsafe {
        // 1. Create Device with default hardware or Adapter 0
        let mut d3d_device: Option<ID3D11Device> = None;
        let mut d3d_context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL_11_0;

        let hr = D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut d3d_device),
            Some(&mut feature_level),
            Some(&mut d3d_context),
        );

        if hr.is_err() {
            println!("D3D11CreateDevice failed: {:?}", hr);
            return;
        }

        let device = d3d_device.unwrap();
        let context = d3d_context.unwrap();

        // 2. Query IDXGIDevice from ID3D11Device
        let dxgi_device: IDXGIDevice = match device.cast() {
            Ok(d) => d,
            Err(e) => {
                println!("device.cast::<IDXGIDevice> failed: {:?}", e);
                return;
            }
        };

        // 3. Get the adapter FROM the DXGI device
        let adapter: IDXGIAdapter = match dxgi_device.GetAdapter() {
            Ok(a) => a,
            Err(e) => {
                println!("dxgi_device.GetAdapter failed: {:?}", e);
                return;
            }
        };

        let desc = adapter.GetDesc().unwrap();
        let name = String::from_utf16_lossy(&desc.Description);
        println!("Active D3D11 Device Adapter: {}", name.trim_matches(char::from(0)));

        // 4. Enumerate outputs from THAT adapter
        for out_idx in 0..4 {
            let output: IDXGIOutput = match adapter.EnumOutputs(out_idx) {
                Ok(o) => o,
                Err(_) => break,
            };

            let out_desc = output.GetDesc().unwrap();
            let out_name = String::from_utf16_lossy(&out_desc.DeviceName);
            println!("Testing Output {}: {}", out_idx, out_name.trim_matches(char::from(0)));

            let output1: IDXGIOutput1 = match output.cast() {
                Ok(o1) => o1,
                Err(e) => {
                    println!("  Failed to cast to IDXGIOutput1: {:?}", e);
                    continue;
                }
            };

            match output1.DuplicateOutput(&device) {
                Ok(dup) => {
                    println!("  >>> SUCCESS! DuplicateOutput worked on output {}!", out_idx);
                    let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
                    let mut desktop_resource: Option<IDXGIResource> = None;
                    let acq = dup.AcquireNextFrame(100, &mut frame_info, &mut desktop_resource);
                    println!("  >>> AcquireNextFrame test result: {:?}", acq);
                    if acq.is_ok() {
                        let _ = dup.ReleaseFrame();
                    }
                }
                Err(e) => {
                    println!("  DuplicateOutput failed on output {}: {:?}", out_idx, e);

                    // Try IDXGIOutput5 DuplicateOutput1 if available
                    if let Ok(output5) = output.cast::<IDXGIOutput5>() {
                        println!("  Trying IDXGIOutput5::DuplicateOutput1...");
                        let formats = [DXGI_FORMAT_B8G8R8A8_UNORM];
                        let dup1 = output5.DuplicateOutput1(&device, 0, &formats);
                        println!("  >>> DuplicateOutput1 result: {:?}", dup1);
                    }
                }
            }
        }
    }
}
