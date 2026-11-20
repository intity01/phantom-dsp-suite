//! Crisp monoline vector UI icons mapped onto a 24x24 viewBox via egui Painter.

use eframe::egui::{
    Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2,
};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    Grid,
    Wave,
    Mic,
    Keys,
    Meter,
    Disk,
    Up,
    Settings,
    Info,
    Search,
    Sync,
    Check,
    Shield,
    Folder,
    Code,
    Alert,
    Zip,
    Export,
    Import,
    Terminal,
    X,
    Copy,
}

pub fn icon_widget(ui: &mut Ui, name: IconName, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    draw_icon(ui.painter(), rect.center(), size, name, color);
}

pub fn icon_text_button(ui: &mut Ui, icon: IconName, text: &str, fg: Color32) -> Response {
    let font = FontId::proportional(12.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let w = 12.0 + 14.0 + 6.0 + galley.size().x + 12.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() {
            Color32::from_rgb(58, 58, 66)
        } else {
            Color32::from_rgb(48, 48, 54)
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(8), bg);
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(18, 18, 18, 18)),
            StrokeKind::Inside,
        );
        draw_icon(
            p,
            Pos2::new(rect.left() + 18.0, rect.center().y),
            14.0,
            icon,
            fg,
        );
        p.galley(
            Pos2::new(rect.left() + 31.0, rect.center().y - galley.size().y * 0.5),
            galley,
            fg,
        );
    }
    resp
}

pub fn icon_only_button(ui: &mut Ui, icon: IconName, size: f32, fg: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size + 6.0), Sense::click());
    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(5),
                Color32::from_rgba_premultiplied(23, 23, 23, 23),
            );
        }
        draw_icon(ui.painter(), rect.center(), size, icon, fg);
    }
    resp
}

pub fn draw_icon(painter: &Painter, center: Pos2, size: f32, name: IconName, color: Color32) {
    let s = size / 24.0;
    let sw = (s * 1.9).max(1.25);
    let st = Stroke::new(sw, color);
    let ox = center.x - size * 0.5;
    let oy = center.y - size * 0.5;
    let pt = |x: f32, y: f32| Pos2::new(ox + x * s, oy + y * s);
    let line =
        |x1: f32, y1: f32, x2: f32, y2: f32| painter.line_segment([pt(x1, y1), pt(x2, y2)], st);
    let poly = |pts: &[(f32, f32)]| {
        let mapped: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(x, y)).collect();
        painter.add(Shape::line(mapped, st));
    };

    match name {
        IconName::Grid => {
            for (x, y) in [(7.0, 7.0), (17.0, 7.0), (7.0, 17.0), (17.0, 17.0)] {
                painter.circle_stroke(pt(x, y), 3.1 * s, st);
            }
        }
        IconName::Wave => poly(&[
            (3.0, 12.0),
            (6.0, 12.0),
            (8.0, 6.0),
            (12.0, 18.0),
            (15.0, 9.0),
            (17.0, 12.0),
            (21.0, 12.0),
        ]),
        IconName::Keys => {
            painter.rect_stroke(
                Rect::from_min_max(pt(3.0, 5.0), pt(21.0, 19.0)),
                CornerRadius::same(2),
                st,
                StrokeKind::Middle,
            );
            for x in [8.0, 12.0, 16.0] {
                line(x, 5.0, x, 19.0);
            }
        }
        IconName::Meter => {
            for (x, top) in [(5.0, 11.0), (10.0, 6.0), (15.0, 14.0), (20.0, 9.0)] {
                line(x, 19.0, x, top);
            }
        }
        IconName::Disk => {
            painter.rect_stroke(
                Rect::from_min_max(pt(4.0, 4.0), pt(20.0, 20.0)),
                CornerRadius::same(3),
                st,
                StrokeKind::Middle,
            );
            painter.circle_stroke(pt(12.0, 12.0), 3.0 * s, st);
        }
        IconName::Mic => {
            painter.rect_stroke(
                Rect::from_min_max(pt(9.0, 3.0), pt(15.0, 14.0)),
                CornerRadius::same((3.0 * s) as u8),
                st,
                StrokeKind::Middle,
            );
            poly(&[
                (6.0, 11.0),
                (6.8, 14.0),
                (9.0, 16.3),
                (12.0, 17.0),
                (15.0, 16.3),
                (17.2, 14.0),
                (18.0, 11.0),
            ]);
            line(12.0, 17.0, 12.0, 21.0);
        }
        IconName::Up => {
            line(12.0, 19.0, 12.0, 6.0);
            poly(&[(7.0, 11.0), (12.0, 6.0), (17.0, 11.0)]);
        }
        IconName::Settings => {
            painter.circle_stroke(pt(12.0, 12.0), 3.0 * s, st);
            for (x1, y1, x2, y2) in [
                (12.0, 2.0, 12.0, 5.0),
                (12.0, 19.0, 12.0, 22.0),
                (2.0, 12.0, 5.0, 12.0),
                (19.0, 12.0, 22.0, 12.0),
                (4.9, 4.9, 7.0, 7.0),
                (17.0, 17.0, 19.1, 19.1),
                (4.9, 19.1, 7.0, 17.0),
                (17.0, 7.0, 19.1, 4.9),
            ] {
                line(x1, y1, x2, y2);
            }
        }
        IconName::Info => {
            painter.circle_stroke(pt(12.0, 12.0), 9.0 * s, st);
            line(12.0, 11.0, 12.0, 16.0);
            painter.circle_filled(pt(12.0, 8.0), (sw * 0.6).max(1.0), color);
        }
        IconName::Search => {
            painter.circle_stroke(pt(11.0, 11.0), 6.2 * s, st);
            line(15.6, 15.6, 20.0, 20.0);
        }
        IconName::Sync => {
            let arc = |a0: f32, a1: f32| {
                let pts: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let a = a0 + (a1 - a0) * (i as f32 / 12.0);
                        pt(12.0 + 7.5 * a.cos(), 12.0 + 7.5 * a.sin())
                    })
                    .collect();
                painter.add(Shape::line(pts, st));
            };
            arc(-PI * 0.88, -0.1);
            arc(0.2, PI * 0.92);
            poly(&[(4.2, 4.5), (4.5, 8.5), (8.5, 8.2)]);
            poly(&[(19.8, 19.5), (19.5, 15.5), (15.5, 15.8)]);
        }
        IconName::Check => poly(&[(5.0, 12.0), (9.5, 16.5), (19.0, 7.0)]),
        IconName::Shield => {
            poly(&[
                (12.0, 3.0),
                (5.0, 6.0),
                (5.0, 12.0),
                (7.5, 16.8),
                (12.0, 21.0),
                (16.5, 16.8),
                (19.0, 12.0),
                (19.0, 6.0),
                (12.0, 3.0),
            ]);
            poly(&[(9.0, 12.0), (11.0, 14.0), (15.5, 9.5)]);
        }
        IconName::Folder => poly(&[
            (3.0, 7.0),
            (4.2, 5.0),
            (9.0, 5.0),
            (11.0, 7.0),
            (20.0, 7.0),
            (21.0, 8.2),
            (21.0, 18.0),
            (20.0, 19.0),
            (4.0, 19.0),
            (3.0, 18.0),
            (3.0, 7.0),
        ]),
        IconName::Code => {
            poly(&[(8.0, 7.0), (3.0, 12.0), (8.0, 17.0)]);
            poly(&[(16.0, 7.0), (21.0, 12.0), (16.0, 17.0)]);
        }
        IconName::Alert => {
            poly(&[(12.0, 3.0), (2.5, 19.5), (21.5, 19.5), (12.0, 3.0)]);
            line(12.0, 10.0, 12.0, 14.2);
            painter.circle_filled(pt(12.0, 17.0), (sw * 0.6).max(1.0), color);
        }
        IconName::Zip => {
            poly(&[
                (6.0, 3.0),
                (14.0, 3.0),
                (18.0, 7.0),
                (18.0, 21.0),
                (6.0, 21.0),
                (6.0, 3.0),
            ]);
            line(11.0, 7.0, 13.0, 7.0);
            line(11.0, 10.0, 13.0, 10.0);
            painter.rect_stroke(
                Rect::from_min_max(pt(11.0, 13.0), pt(13.0, 16.0)),
                CornerRadius::ZERO,
                st,
                StrokeKind::Middle,
            );
        }
        IconName::Export => {
            line(12.0, 15.0, 12.0, 3.0);
            poly(&[(7.0, 8.0), (12.0, 3.0), (17.0, 8.0)]);
            poly(&[(5.0, 14.0), (5.0, 20.0), (19.0, 20.0), (19.0, 14.0)]);
        }
        IconName::Import => {
            line(12.0, 3.0, 12.0, 15.0);
            poly(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)]);
            poly(&[(5.0, 14.0), (5.0, 20.0), (19.0, 20.0), (19.0, 14.0)]);
        }
        IconName::Terminal => {
            painter.rect_stroke(
                Rect::from_min_max(pt(3.0, 4.0), pt(21.0, 20.0)),
                CornerRadius::same(2),
                st,
                StrokeKind::Middle,
            );
            poly(&[(7.0, 9.0), (10.5, 12.0), (7.0, 15.0)]);
            line(12.5, 15.0, 17.0, 15.0);
        }
        IconName::X => {
            line(6.0, 6.0, 18.0, 18.0);
            line(18.0, 6.0, 6.0, 18.0);
        }
        IconName::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(pt(9.0, 9.0), pt(20.0, 20.0)),
                CornerRadius::same(2),
                st,
                StrokeKind::Middle,
            );
            poly(&[
                (5.0, 15.0),
                (4.0, 15.0),
                (4.0, 4.0),
                (15.0, 4.0),
                (15.0, 5.0),
            ]);
        }
    }
}
