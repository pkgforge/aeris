//! What aeris has to draw for itself when the compositor will not.
//!
//! GNOME and anything else refusing `xdg-decoration` hands the window back
//! undecorated, so without this there is no way to move, resize or close it.
//! The controls sit in the header that is there anyway; only the edges are
//! added on top.

use gpui::*;

use crate::{app::App, styles, theme};

/// How wide a window edge has to be to be grabbed.
const GRAB: f32 = 6.0;

/// Which of the three the button is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

impl Control {
    fn name(self) -> &'static str {
        match self {
            Control::Minimize => "window-minimize",
            Control::Maximize => "window-maximize",
            Control::Close => "window-close",
        }
    }
}

impl App {
    /// The three window controls, or nothing when the compositor draws its
    /// own. They live in the header rather than in a bar of their own, so an
    /// undecorated window costs no extra row.
    pub fn window_controls(
        &self,
        theme: &theme::Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !matches!(window.window_decorations(), Decorations::Client { .. }) {
            return None;
        }

        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(styles::spacing::XXXS))
                .ml(px(styles::spacing::SM))
                .child(self.window_button(Control::Minimize, theme, cx))
                .child(self.window_button(Control::Maximize, theme, cx))
                .child(self.window_button(Control::Close, theme, cx)),
        )
    }

    /// Whether this window has to be moved and resized by hand.
    pub fn draws_own_decorations(window: &Window) -> bool {
        matches!(window.window_decorations(), Decorations::Client { .. })
    }

    /// One of the three window controls.
    ///
    /// The dash and the square are drawn rather than written: taken from a
    /// font they arrive at different weights and sit at different heights,
    /// which is what makes a hand-drawn titlebar look hand-drawn.
    fn window_button(
        &self,
        control: Control,
        theme: &theme::Theme,
        _cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let hover = theme.hover;
        let danger = theme.danger;
        let ink = theme.text_muted;

        let mark = match control {
            Control::Minimize => div().w(px(9.0)).h(px(1.0)).bg(ink),
            Control::Maximize => div()
                .size(px(9.0))
                .border_1()
                .border_color(ink)
                .rounded(px(1.0)),
            // The one mark that cannot be drawn from boxes, since gpui only
            // rotates svg. Sized to sit at the same weight as the other two.
            Control::Close => div().text_size(px(12.0)).text_color(ink).child("\u{2715}"),
        };

        div()
            .id(control.name())
            .size(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(styles::radius::SM))
            .cursor_pointer()
            .hover(move |style| match control {
                Control::Close => style.bg(danger),
                _ => style.bg(hover),
            })
            // The bar moves the window on mouse down, so a button has to say
            // the press was for it and not for the bar behind it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(move |_, window, _| match control {
                Control::Minimize => window.minimize_window(),
                Control::Maximize => window.zoom_window(),
                Control::Close => window.remove_window(),
            })
            .child(mark)
    }

    /// The edges of an undecorated window, so it can still be resized.
    ///
    /// An edge the compositor has tiled is left alone: dragging it there does
    /// nothing, and offering the handle would only mislead.
    pub fn render_resize_edges(&self, window: &Window) -> Option<Div> {
        let Decorations::Client { tiling } = window.window_decorations() else {
            return None;
        };

        let grab = px(GRAB);
        let mut edges = div().absolute().size_full().occlude();

        for (edge, tiled) in [
            (ResizeEdge::Top, tiling.top),
            (ResizeEdge::Bottom, tiling.bottom),
            (ResizeEdge::Left, tiling.left),
            (ResizeEdge::Right, tiling.right),
            (ResizeEdge::TopLeft, tiling.top || tiling.left),
            (ResizeEdge::TopRight, tiling.top || tiling.right),
            (ResizeEdge::BottomLeft, tiling.bottom || tiling.left),
            (ResizeEdge::BottomRight, tiling.bottom || tiling.right),
        ] {
            if tiled {
                continue;
            }

            let mut handle = div().absolute();
            let corner = px(GRAB * 2.0);

            handle = match edge {
                ResizeEdge::Top => handle.top_0().left_0().right_0().h(grab),
                ResizeEdge::Bottom => handle.bottom_0().left_0().right_0().h(grab),
                ResizeEdge::Left => handle.left_0().top_0().bottom_0().w(grab),
                ResizeEdge::Right => handle.right_0().top_0().bottom_0().w(grab),
                ResizeEdge::TopLeft => handle.top_0().left_0().size(corner),
                ResizeEdge::TopRight => handle.top_0().right_0().size(corner),
                ResizeEdge::BottomLeft => handle.bottom_0().left_0().size(corner),
                ResizeEdge::BottomRight => handle.bottom_0().right_0().size(corner),
            };

            edges = edges.child(
                handle.on_mouse_down(MouseButton::Left, move |_, window, _| {
                    window.start_window_resize(edge);
                }),
            );
        }

        Some(edges)
    }
}
