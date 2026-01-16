//! DirectComposition setup for WebView2 CompositionController.

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::Interface;

/// DirectComposition resources for visual hosting.
pub struct CompositionHost {
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    root_visual: IDCompositionVisual,
}

impl CompositionHost {
    /// Creates DirectComposition device and visual tree for the given window.
    pub fn new(hwnd: HWND) -> windows::core::Result<Self> {
        unsafe {
            // Create D3D11 device (required for DComposition)
            let mut d3d_device = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                None,
            )?;
            let d3d_device = d3d_device.unwrap();

            // Get DXGI device for DComposition
            let dxgi_device: IDXGIDevice = d3d_device.cast()?;

            // Create DComposition device
            let dcomp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;

            // Create composition target for the window
            let target = dcomp_device.CreateTargetForHwnd(hwnd, true)?;

            // Create root visual
            let root_visual = dcomp_device.CreateVisual()?;

            // Set the root visual on the target
            target.SetRoot(&root_visual)?;

            // Commit the composition
            dcomp_device.Commit()?;

            Ok(Self {
                device: dcomp_device,
                _target: target,
                root_visual,
            })
        }
    }

    /// Returns the root visual for WebView2 to render into.
    pub fn root_visual(&self) -> &IDCompositionVisual {
        &self.root_visual
    }

    /// Sets the position of the root visual within the window.
    pub fn set_offset(&self, x: i32, y: i32) -> windows::core::Result<()> {
        unsafe {
            self.root_visual.SetOffsetX2(x as f32)?;
            self.root_visual.SetOffsetY2(y as f32)?;
            Ok(())
        }
    }

    /// Commits any pending composition changes.
    pub fn commit(&self) -> windows::core::Result<()> {
        unsafe { self.device.Commit() }
    }
}
