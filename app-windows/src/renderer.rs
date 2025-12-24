//! Direct2D rendering infrastructure.

use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT, D2D_SIZE_U},
                D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
        UI::WindowsAndMessaging::GetClientRect,
    },
};

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// Direct2D renderer for the application window.
pub struct Renderer {
    factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1HwndRenderTarget>,
    text_format: IDWriteTextFormat,
    // Cached brushes
    bg_brush: Option<ID2D1SolidColorBrush>,
    text_brush: Option<ID2D1SolidColorBrush>,
}

impl Renderer {
    /// Creates a new renderer.
    pub fn new() -> Result<Self> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };

        let text_format = unsafe {
            dwrite_factory.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                14.0,
                windows::core::w!("en-US"),
            )?
        };

        Ok(Self {
            factory,
            dwrite_factory,
            render_target: None,
            text_format,
            bg_brush: None,
            text_brush: None,
        })
    }

    /// Ensures the render target exists and is sized correctly.
    pub fn ensure_target(&mut self, hwnd: HWND) -> Result<()> {
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect)? };

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Ok(());
        }

        // Check if we need to create or resize
        let needs_create = match &self.render_target {
            None => true,
            Some(rt) => {
                let size = unsafe { rt.GetSize() };
                size.width as u32 != width || size.height as u32 != height
            }
        };

        if needs_create {
            // Drop old resources
            self.bg_brush = None;
            self.text_brush = None;
            self.render_target = None;

            let render_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: Default::default(),
            };

            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };

            let rt = unsafe {
                self.factory
                    .CreateHwndRenderTarget(&render_props, &hwnd_props)?
            };

            // Create brushes
            let bg_brush = unsafe {
                rt.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    },
                    None,
                )?
            };

            let text_brush = unsafe {
                rt.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.9,
                        g: 0.9,
                        b: 0.9,
                        a: 1.0,
                    },
                    None,
                )?
            };

            self.bg_brush = Some(bg_brush);
            self.text_brush = Some(text_brush);
            self.render_target = Some(rt);
        }

        Ok(())
    }

    /// Renders the key list.
    pub fn render(&self, keys: &[keva_core::types::Key]) -> Result<()> {
        let Some(rt) = &self.render_target else {
            return Ok(());
        };
        let Some(_bg_brush) = &self.bg_brush else {
            return Ok(());
        };
        let Some(text_brush) = &self.text_brush else {
            return Ok(());
        };

        unsafe {
            rt.BeginDraw();

            // Clear background
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.1,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            }));

            let size = rt.GetSize();

            // Draw key list
            let mut y = 10.0f32;
            let line_height = 24.0f32;

            for key in keys.iter().take(20) {
                // Limit visible keys for now
                let text: Vec<u16> = key.as_str().encode_utf16().collect();

                let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: 10.0,
                    top: y,
                    right: size.width - 10.0,
                    bottom: y + line_height,
                };

                rt.DrawText(
                    &text,
                    &self.text_format,
                    &rect,
                    text_brush,
                    Default::default(),
                    Default::default(),
                );

                y += line_height;
            }

            // Show count if there are more keys
            if keys.len() > 20 {
                let more = format!("... and {} more", keys.len() - 20);
                let text: Vec<u16> = more.encode_utf16().collect();

                let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: 10.0,
                    top: y,
                    right: size.width - 10.0,
                    bottom: y + line_height,
                };

                rt.DrawText(
                    &text,
                    &self.text_format,
                    &rect,
                    text_brush,
                    Default::default(),
                    Default::default(),
                );
            }

            rt.EndDraw(None, None)?;
        }

        Ok(())
    }

    /// Resizes the render target.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(rt) = &self.render_target {
            unsafe {
                rt.Resize(&D2D_SIZE_U { width, height })?;
            }
        }
        Ok(())
    }
}
