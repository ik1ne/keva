use crate::theme::{
    BG_COLOR, DRAG_BORDER_PX, INPUT_BG_COLOR, LEFT_PANEL_DEFAULT_WIDTH, LEFT_PANEL_MIN_WIDTH,
    PANEL_BORDER_COLOR, SEARCH_BAR_HEIGHT, TEXT_COLOR,
};
use gpui::{Context, CursorStyle, Entity, Window, WindowControlArea, div, prelude::*, px, rgb};
use gpui_component::input::{Input, InputState};
use gpui_component::resizable::{h_resizable, resizable_panel};

pub struct KevaApp {
    search_input: Entity<InputState>,
}

impl KevaApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|inner_cx| InputState::new(window, inner_cx).placeholder("Search keys..."));
        Self { search_input }
    }
}

impl Render for KevaApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(BG_COLOR))
            .text_color(rgb(TEXT_COLOR))
            .relative()
            // Drag borders (absolute positioned overlays)
            .child(self.render_drag_borders())
            // Main content
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(self.render_search_bar())
                    .child(self.render_main_content()),
            )
    }
}

impl KevaApp {
    fn render_drag_borders(&self) -> impl IntoElement {
        let border = px(DRAG_BORDER_PX);

        div()
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            // Top border
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(border)
                    .cursor(CursorStyle::OpenHand)
                    .window_control_area(WindowControlArea::Drag),
            )
            // Bottom border
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(border)
                    .cursor(CursorStyle::OpenHand)
                    .window_control_area(WindowControlArea::Drag),
            )
            // Left border
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(border)
                    .h_full()
                    .cursor(CursorStyle::OpenHand)
                    .window_control_area(WindowControlArea::Drag),
            )
            // Right border
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(border)
                    .h_full()
                    .cursor(CursorStyle::OpenHand)
                    .window_control_area(WindowControlArea::Drag),
            )
    }

    fn render_search_bar(&self) -> impl IntoElement {
        div()
            .w_full()
            .h(px(SEARCH_BAR_HEIGHT))
            .bg(rgb(INPUT_BG_COLOR))
            .border_b_1()
            .border_color(rgb(PANEL_BORDER_COLOR))
            .flex()
            .items_center()
            .px_2()
            .gap_2()
            // Search icon (drag handle for window)
            .child(
                div()
                    .child("🔍")
                    .cursor(CursorStyle::OpenHand)
                    .window_control_area(WindowControlArea::Drag),
            )
            // Search input
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.search_input).cleanable(true)),
            )
    }

    fn render_main_content(&self) -> impl IntoElement {
        div().flex_1().child(
            h_resizable("main-panels")
                // Left panel (key list)
                .child(
                    resizable_panel()
                        .size(px(LEFT_PANEL_DEFAULT_WIDTH))
                        .size_range(px(LEFT_PANEL_MIN_WIDTH)..px(f32::MAX))
                        .child(self.render_key_list()),
                )
                // Right panel (inspector)
                .child(resizable_panel().child(self.render_inspector())),
        )
    }

    fn render_key_list(&self) -> impl IntoElement {
        div()
            .size_full()
            .border_r_1()
            .border_color(rgb(PANEL_BORDER_COLOR))
            .p_2()
            .flex()
            .flex_col()
            .child(div().text_xl().child("Keys"))
            .child(
                div()
                    .text_color(rgb(0x888888))
                    .child("(No keys loaded - M1 placeholder)"),
            )
    }

    fn render_inspector(&self) -> impl IntoElement {
        div()
            .size_full()
            .p_2()
            .flex()
            .flex_col()
            .child(div().text_xl().child("Inspector"))
            .child(
                div()
                    .text_color(rgb(0x888888))
                    .child("(Select a key to view - M1 placeholder)"),
            )
    }
}
