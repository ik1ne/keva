//! Direct2D rendering infrastructure with DirectComposition for flicker-free resize.

use super::theme::{
    COLOR_BG, COLOR_DIVIDER, COLOR_LEFT_PANE_BG, COLOR_RIGHT_PANE_BG, COLOR_SEARCH_BAR_BG,
    COLOR_SEARCH_ICON, COLOR_SEARCH_ICON_BG,
};
use crate::ui::{Layout, Rect};
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
                D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1,
                ID2D1SolidColorBrush,
            },
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
            },
            DirectComposition::{DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget},
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWriteCreateFactory,
                IDWriteFactory, IDWriteTextFormat,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_SCALING_STRETCH,
                DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
    },
    core::{Interface, Result, w},
};

const SEARCH_TEXT_SIZE: f32 = 16.0;
const SEARCH_ICON_GLYPH: &str = "🔍";

/// Direct2D renderer using DirectComposition for flicker-free resize.
pub struct Renderer {
    // Direct2D
    d2d_factory: ID2D1Factory1,
    d2d_device: ID2D1Device,
    d2d_context: ID2D1DeviceContext,
    target_bitmap: Option<ID2D1Bitmap1>,

    // DXGI swap chain
    swap_chain: IDXGISwapChain1,

    // DirectComposition
    _dcomp_device: IDCompositionDevice,
    _dcomp_target: IDCompositionTarget,

    // DirectWrite
    dwrite_factory: IDWriteFactory,
    text_format: IDWriteTextFormat,
}

impl Renderer {
    /// Creates a new renderer bound to the given window.
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        // Step 1: Create D3D11 device with BGRA support (required for D2D)
        let mut d3d_device = None;
        unsafe {
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
        }
        let d3d_device = d3d_device.unwrap();

        // Step 2: Get DXGI device from D3D11 device
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;

        // Step 3: Create D2D1 factory (version 1.1)
        let d2d_factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        // Step 4: Create D2D1 device from DXGI device
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device)? };

        // Step 5: Create D2D1 device context
        let d2d_context =
            unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };

        // Step 6: Get DXGI factory and create swap chain for composition
        let dxgi_adapter = unsafe { dxgi_device.GetAdapter()? };
        let dxgi_factory: IDXGIFactory2 = unsafe { dxgi_adapter.GetParent()? };

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            ..Default::default()
        };

        let swap_chain = unsafe {
            dxgi_factory.CreateSwapChainForComposition(&d3d_device, &swap_chain_desc, None)?
        };

        // Step 7: Create DirectComposition device
        let dcomp_device: IDCompositionDevice =
            unsafe { DCompositionCreateDevice(&dxgi_device)? };

        // Step 8: Create composition target and visual, bind swap chain
        let dcomp_target = unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true)? };
        let dcomp_visual = unsafe { dcomp_device.CreateVisual()? };

        unsafe {
            dcomp_visual.SetContent(&swap_chain)?;
            dcomp_target.SetRoot(&dcomp_visual)?;
            dcomp_device.Commit()?;
        }

        // Step 9: Create DirectWrite factory and text format
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };

        let text_format = unsafe {
            dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                SEARCH_TEXT_SIZE,
                w!("en-US"),
            )?
        };

        unsafe {
            text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }

        let mut renderer = Self {
            d2d_factory,
            d2d_device,
            d2d_context,
            target_bitmap: None,
            swap_chain,
            _dcomp_device: dcomp_device,
            _dcomp_target: dcomp_target,
            dwrite_factory,
            text_format,
        };

        // Step 10: Create D2D bitmap from swap chain back buffer
        renderer.create_target_bitmap()?;

        Ok(renderer)
    }

    /// Creates D2D bitmap from swap chain back buffer.
    fn create_target_bitmap(&mut self) -> Result<()> {
        let back_buffer: windows::Win32::Graphics::Dxgi::IDXGISurface =
            unsafe { self.swap_chain.GetBuffer(0)? };

        let bitmap_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };

        let bitmap =
            unsafe { self.d2d_context.CreateBitmapFromDxgiSurface(&back_buffer, Some(&bitmap_props))? };

        unsafe { self.d2d_context.SetTarget(&bitmap) };
        self.target_bitmap = Some(bitmap);

        Ok(())
    }

    /// Resizes the swap chain and recreates the render target.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Release current target
        self.target_bitmap = None;
        unsafe { self.d2d_context.SetTarget(None) };

        // Resize swap chain buffers
        unsafe {
            self.swap_chain.ResizeBuffers(
                0, // Keep buffer count
                width,
                height,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG::default(),
            )?;
        }

        // Recreate bitmap from new back buffer
        self.create_target_bitmap()?;

        Ok(())
    }

    /// Renders the window content with the given layout.
    pub fn render(&self, layout: &Layout) -> Result<()> {
        if self.target_bitmap.is_none() {
            return Ok(());
        }

        unsafe {
            self.d2d_context.BeginDraw();
            self.d2d_context.Clear(Some(&COLOR_BG));

            // Draw search bar background
            self.fill_rect(&layout.search_bar, &COLOR_SEARCH_BAR_BG)?;

            // Draw search icon background
            self.fill_rounded_rect(&layout.search_icon, 4.0, &COLOR_SEARCH_ICON_BG)?;

            // Draw search icon (magnifying glass emoji)
            self.draw_centered_text(&layout.search_icon, SEARCH_ICON_GLYPH, &COLOR_SEARCH_ICON)?;

            // Draw left pane background
            self.fill_rect(&layout.left_pane, &COLOR_LEFT_PANE_BG)?;

            // Draw divider
            self.fill_rect(&layout.divider, &COLOR_DIVIDER)?;

            // Draw right pane background
            self.fill_rect(&layout.right_pane, &COLOR_RIGHT_PANE_BG)?;

            self.d2d_context.EndDraw(None, None)?;

            // Present with vsync
            self.swap_chain
                .Present(1, windows::Win32::Graphics::Dxgi::DXGI_PRESENT::default())
                .ok()?;
        }

        Ok(())
    }

    /// Creates a solid color brush.
    fn create_brush(&self, color: &D2D1_COLOR_F) -> Result<ID2D1SolidColorBrush> {
        unsafe { self.d2d_context.CreateSolidColorBrush(color, None) }
    }

    /// Fills a rectangle with the given color.
    fn fill_rect(&self, rect: &Rect, color: &D2D1_COLOR_F) -> Result<()> {
        let brush = self.create_brush(color)?;
        let d2d_rect = rect_to_d2d(rect);
        unsafe { self.d2d_context.FillRectangle(&d2d_rect, &brush) };
        Ok(())
    }

    /// Fills a rounded rectangle with the given color.
    fn fill_rounded_rect(&self, rect: &Rect, radius: f32, color: &D2D1_COLOR_F) -> Result<()> {
        let brush = self.create_brush(color)?;
        let d2d_rect = rect_to_d2d(rect);
        let rounded_rect = windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
            rect: d2d_rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { self.d2d_context.FillRoundedRectangle(&rounded_rect, &brush) };
        Ok(())
    }

    /// Draws centered text in a rectangle (for icons).
    fn draw_centered_text(&self, rect: &Rect, text: &str, color: &D2D1_COLOR_F) -> Result<()> {
        let brush = self.create_brush(color)?;
        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

        // Create a centered text format for the icon
        let centered_format = unsafe {
            self.dwrite_factory.CreateTextFormat(
                w!("Segoe UI Emoji"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                18.0,
                w!("en-US"),
            )?
        };
        unsafe {
            centered_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            centered_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }

        let d2d_rect = rect_to_d2d(rect);
        unsafe {
            self.d2d_context.DrawText(
                &text_wide[..text_wide.len() - 1],
                &centered_format,
                &d2d_rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                Default::default(),
            );
        }
        Ok(())
    }
}

/// Converts a Rect to a D2D_RECT_F.
fn rect_to_d2d(rect: &Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x,
        top: rect.y,
        right: rect.right(),
        bottom: rect.bottom(),
    }
}
