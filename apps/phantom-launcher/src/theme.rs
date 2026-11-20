//! Apple Blue-Zinc Dark UI Theme (`#18181B` window, `#1F1F23` surface, `#0A84FF` accent).

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, Pos2, Response, RichText, Sense, Stroke, Ui, Vec2,
    Visuals,
};

// Depth comes from tone steps, not outlines: window → shell → pane → card.
pub const BG_WINDOW: Color32 = Color32::from_rgb(24, 24, 27); // #18181B
pub const BG_SHELL: Color32 = Color32::from_rgb(28, 28, 32); // wraps Hub + Details
pub const BG_SURFACE: Color32 = Color32::from_rgb(34, 34, 39); // each pane
pub const BG_CARD: Color32 = Color32::from_rgb(42, 42, 48);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(49, 49, 56);
pub const BG_CARD_SELECTED: Color32 = Color32::from_rgb(32, 46, 64);

// Concentric corners: shell radius = pane radius + the gap between them.
pub const SHELL_GAP: i8 = 8;
pub const PANE_RADIUS: u8 = 18;
pub const SHELL_RADIUS: u8 = PANE_RADIUS + SHELL_GAP as u8;

pub const FILL_1: Color32 = Color32::from_rgba_premultiplied(14, 14, 14, 14);
pub const FILL_2: Color32 = Color32::from_rgba_premultiplied(23, 23, 23, 23);
pub const FILL_3: Color32 = Color32::from_rgba_premultiplied(36, 36, 36, 36);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 18);
pub const BORDER_LIGHT: Color32 = Color32::from_rgba_premultiplied(32, 32, 32, 32);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 245, 247); // #F5F5F7
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(163, 163, 174);
pub const TEXT_TERTIARY: Color32 = Color32::from_rgb(112, 112, 122);

pub const ACCENT: Color32 = Color32::from_rgb(10, 132, 255); // #0A84FF
pub const ACCENT_TOP: Color32 = Color32::from_rgb(45, 151, 255); // #2D97FF
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(94, 176, 255); // #5EB0FF
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(2, 21, 41, 41);

pub const GREEN: Color32 = Color32::from_rgb(48, 209, 88); // #30D158
pub const GREEN_SOFT: Color32 = Color32::from_rgba_premultiplied(7, 30, 12, 36);
pub const ORANGE: Color32 = Color32::from_rgb(255, 159, 10); // #FF9F0A
pub const ORANGE_SOFT: Color32 = Color32::from_rgba_premultiplied(31, 19, 1, 31);
pub const RED: Color32 = Color32::from_rgb(255, 69, 58); // #FF453A

pub fn apply_suite_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_WINDOW;
    visuals.window_fill = BG_SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(20, 20, 24);
    visuals.faint_bg_color = BG_CARD;

    visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_SECONDARY);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(46, 46, 52);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(46, 46, 52);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 56, 64);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(56, 56, 64);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_LIGHT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(10, 132, 255, 115);
    visuals.selection.stroke = Stroke::new(1.5_f32, ACCENT);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.scroll.bar_width = 6.0;
    ctx.set_style(style);
}

/// Window background plus the soft accent glow from the design
/// (`radial-gradient(1200px 600px at -10% -20%, accent 8%, transparent 60%)`).
/// Painted first on the background layer so every panel (with a transparent fill) sits on it.
pub fn paint_window_backdrop(ctx: &egui::Context) {
    let rect = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::background());
    p.rect_filled(rect, CornerRadius::ZERO, BG_WINDOW);
    let c = Pos2::new(
        rect.left() - rect.width() * 0.10,
        rect.top() - rect.height() * 0.20,
    );
    let (rx, ry) = (720.0_f32, 360.0_f32);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, Color32::from_rgba_unmultiplied(10, 132, 255, 20));
    let n = 72_u32;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(
            c + Vec2::new(a.cos() * rx, a.sin() * ry),
            Color32::TRANSPARENT,
        );
    }
    for i in 1..=n {
        mesh.add_triangle(0, i, i + 1);
    }
    p.add(mesh);
}

/// One rounded, borderless shell behind the Hub and Details panes (no outline:
/// the tone step from the window is enough to separate it).
pub fn paint_content_shell(ctx: &egui::Context, left: f32, top: f32, inset: i8) {
    let r = ctx.screen_rect();
    let shell = egui::Rect::from_min_max(
        Pos2::new(r.left() + left, r.top() + top),
        Pos2::new(r.right() - inset as f32, r.bottom() - inset as f32),
    );
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        shell,
        CornerRadius::same(SHELL_RADIUS),
        BG_SHELL,
    );
}

/// Vertical gradient from `color` (opaque) to transparent, used to fade scrolled content
/// into its pane. `opaque_at_top` picks which edge is solid.
pub fn paint_vertical_fade(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: Color32,
    opaque_at_top: bool,
) {
    let (top, bottom) = if opaque_at_top {
        (color, Color32::TRANSPARENT)
    } else {
        (Color32::TRANSPARENT, color)
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}

pub fn draw_pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(11.0))
                    .strong()
                    .color(fg),
            );
        });
}

pub fn primary_blue_button(ui: &mut Ui, text: &str, full_width: bool) -> Response {
    let min_w = if full_width {
        ui.available_width()
    } else {
        112.0
    };
    let height = if full_width { 38.0 } else { 30.0 };
    let radius = if full_width { 10 } else { 8 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(min_w, height), Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let bg = if !enabled {
            Color32::from_rgba_unmultiplied(10, 132, 255, 95)
        } else if resp.is_pointer_button_down_on() {
            Color32::from_rgb(8, 108, 214)
        } else if resp.hovered() {
            ACCENT_TOP
        } else {
            ACCENT
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(radius), bg);
        if enabled {
            p.line_segment(
                [
                    Pos2::new(rect.left() + 8.0, rect.top() + 1.0),
                    Pos2::new(rect.right() - 8.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 52)),
            );
        }
        let fg = if enabled {
            Color32::WHITE
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 140)
        };
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(if full_width { 13.5 } else { 12.5 }),
            fg,
        );
    }
    resp
}

pub fn capsule_button(ui: &mut Ui, text: &str, accent_style: bool) -> Response {
    let (bg, fg) = if accent_style {
        (Color32::from_rgb(48, 50, 58), ACCENT_TEXT)
    } else {
        (Color32::TRANSPARENT, TEXT_SECONDARY)
    };
    let hover_bg = Color32::from_rgb(60, 62, 72);
    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .font(FontId::proportional(12.0))
                .strong()
                .color(if ui.is_enabled() { fg } else { TEXT_TERTIARY }),
        )
        .fill(bg)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(14))
        .min_size(Vec2::new(72.0, 28.0)),
    );
    if resp.hovered() && ui.is_enabled() {
        ui.painter()
            .rect_filled(resp.rect, CornerRadius::same(14), hover_bg);
        ui.painter().text(
            resp.rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(12.0),
            if accent_style {
                ACCENT_TEXT
            } else {
                TEXT_PRIMARY
            },
        );
    }
    resp
}

pub fn quiet_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .font(FontId::proportional(12.5))
                .strong()
                .color(TEXT_PRIMARY),
        )
        .fill(Color32::from_rgb(48, 48, 54))
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 30.0)),
    )
}

pub fn draw_toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let track = if *on {
            ACCENT
        } else {
            Color32::from_rgb(62, 62, 68)
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(12), track);
        let cx = if *on {
            rect.right() - 12.0
        } else {
            rect.left() + 12.0
        };
        p.circle_filled(
            Pos2::new(cx, rect.center().y + 1.0),
            10.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 80),
        );
        p.circle_filled(Pos2::new(cx, rect.center().y), 10.0, Color32::WHITE);
    }
    resp
}
