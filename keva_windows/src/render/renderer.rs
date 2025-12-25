//! Direct2D rendering infrastructure.

use super::theme::{
    COLOR_BG, COLOR_DIVIDER, COLOR_LEFT_PANE_BG, COLOR_RIGHT_PANE_BG, COLOR_SEARCH_BAR_BG,
    COLOR_SEARCH_ICON, COLOR_SEARCH_ICON_BG,
};
use crate::ui::{Layout, Rect};
use windows::{
    Win32::{
        Foundation::{D2DERR_RECREATE_TARGET, HWND},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
                    D2D1_PIXEL_FORMAT,
                },
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory, ID2D1Factory,
                ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT_LEADING, DWriteCreateFactory, IDWriteFactory,
                IDWriteTextFormat,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
        UI::WindowsAndMessaging::GetClientRect,
    },
    core::{Result, w},
};

const SEARCH_TEXT_SIZE: f32 = 16.0;
const SEARCH_ICON_GLYPH: &str = "🔍";

/// Direct2D renderer for the application window.
pub struct Renderer {
    factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    text_format: IDWriteTextFormat,
    render_target: Option<ID2D1HwndRenderTarget>,
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

        Ok(Self {
            factory,
            dwrite_factory,
            text_format,
            render_target: None,
        })
    }

    /// Ensures the render target exists and is sized correctly.
    fn ensure_target(&mut self, hwnd: HWND) -> Result<()> {
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect)? };

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Ok(());
        }

        // Resize existing target if size changed
        if let Some(rt) = &self.render_target {
            let size = unsafe { rt.GetSize() };
            if size.width as u32 != width || size.height as u32 != height {
                unsafe { rt.Resize(&D2D_SIZE_U { width, height })? };
            }
            return Ok(());
        }

        // Create new target only if none exists
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

        self.render_target = Some(unsafe {
            self.factory
                .CreateHwndRenderTarget(&render_props, &hwnd_props)?
        });

        Ok(())
    }

    /// Renders the window content with the given layout.
    ///
    /// The search bar EDIT control renders itself as a child window.
    /// Handles device loss by recreating the render target and retrying.
    pub fn render(&mut self, hwnd: HWND, layout: &Layout) -> Result<()> {
        self.ensure_target(hwnd)?;

        let Some(rt) = &self.render_target else {
            return Ok(());
        };

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&COLOR_BG));

            // Draw search bar background
            self.fill_rect(rt, &layout.search_bar, &COLOR_SEARCH_BAR_BG)?;

            // Draw search icon background
            self.fill_rounded_rect(rt, &layout.search_icon, 4.0, &COLOR_SEARCH_ICON_BG)?;

            // Draw search icon (magnifying glass emoji)
            self.draw_centered_text(
                rt,
                &layout.search_icon,
                SEARCH_ICON_GLYPH,
                &COLOR_SEARCH_ICON,
            )?;

            // NOTE: search_input area is NOT painted here - the EDIT child window handles it

            // Draw left pane background
            self.fill_rect(rt, &layout.left_pane, &COLOR_LEFT_PANE_BG)?;

            // Draw divider
            self.fill_rect(rt, &layout.divider, &COLOR_DIVIDER)?;

            // Draw right pane background
            self.fill_rect(rt, &layout.right_pane, &COLOR_RIGHT_PANE_BG)?;

            match rt.EndDraw(None, None) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == D2DERR_RECREATE_TARGET => {
                    // Device lost - recreate target and retry
                    self.render_target = None;
                    self.render(hwnd, layout)
                }
                Err(e) => Err(e),
            }
        }
    }

    /// Creates a solid color brush.
    fn create_brush(
        &self,
        rt: &ID2D1HwndRenderTarget,
        color: &D2D1_COLOR_F,
    ) -> Result<ID2D1SolidColorBrush> {
        unsafe { rt.CreateSolidColorBrush(color, None) }
    }

    /// Fills a rectangle with the given color.
    fn fill_rect(
        &self,
        rt: &ID2D1HwndRenderTarget,
        rect: &Rect,
        color: &D2D1_COLOR_F,
    ) -> Result<()> {
        let brush = self.create_brush(rt, color)?;
        let d2d_rect = rect_to_d2d(rect);
        unsafe { rt.FillRectangle(&d2d_rect, &brush) };
        Ok(())
    }

    /// Fills a rounded rectangle with the given color.
    fn fill_rounded_rect(
        &self,
        rt: &ID2D1HwndRenderTarget,
        rect: &Rect,
        radius: f32,
        color: &D2D1_COLOR_F,
    ) -> Result<()> {
        let brush = self.create_brush(rt, color)?;
        let d2d_rect = rect_to_d2d(rect);
        let rounded_rect = windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
            rect: d2d_rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { rt.FillRoundedRectangle(&rounded_rect, &brush) };
        Ok(())
    }

    /// Draws text in a rectangle.
    fn draw_text(
        &self,
        rt: &ID2D1HwndRenderTarget,
        rect: &Rect,
        text: &str,
        color: &D2D1_COLOR_F,
    ) -> Result<()> {
        let brush = self.create_brush(rt, color)?;
        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let d2d_rect = rect_to_d2d_with_padding(rect, 8.0);
        unsafe {
            rt.DrawText(
                &text_wide[..text_wide.len() - 1],
                &self.text_format,
                &d2d_rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                Default::default(),
            );
        }
        Ok(())
    }

    /// Draws centered text in a rectangle (for icons).
    fn draw_centered_text(
        &self,
        rt: &ID2D1HwndRenderTarget,
        rect: &Rect,
        text: &str,
        color: &D2D1_COLOR_F,
    ) -> Result<()> {
        let brush = self.create_brush(rt, color)?;
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
            centered_format.SetTextAlignment(
                windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            centered_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }

        let d2d_rect = rect_to_d2d(rect);
        unsafe {
            rt.DrawText(
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

/// Converts a Rect to a D2D_RECT_F with horizontal padding.
fn rect_to_d2d_with_padding(rect: &Rect, padding: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x + padding,
        top: rect.y,
        right: rect.right() - padding,
        bottom: rect.bottom(),
    }
}
