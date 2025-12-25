//! Direct2D rendering infrastructure.

use super::theme::COLOR_BG;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT},
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory,
                ID2D1Factory, ID2D1HwndRenderTarget,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
        UI::WindowsAndMessaging::GetClientRect,
    },
    core::Result,
};

/// Direct2D renderer for the application window.
pub struct Renderer {
    factory: ID2D1Factory,
    render_target: Option<ID2D1HwndRenderTarget>,
}

impl Renderer {
    /// Creates a new renderer.
    pub fn new() -> Result<Self> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        Ok(Self {
            factory,
            render_target: None,
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
        let needs_create = self.render_target.as_ref().is_none_or(|rt| {
            let size = unsafe { rt.GetSize() };
            size.width as u32 != width || size.height as u32 != height
        });

        if needs_create {
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

            self.render_target = Some(rt);
        }

        Ok(())
    }

    /// Renders the window content.
    pub fn render(&self) -> Result<()> {
        let Some(rt) = &self.render_target else {
            return Ok(());
        };

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&COLOR_BG));
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
