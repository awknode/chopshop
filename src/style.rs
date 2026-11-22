//! Awknode street style, shared look with KickNull (copied from KickNull's editor).
//! Concrete wall, spray-paint tag, caution tape, tape-label knobs, sticker toggles.

use nice_plug::prelude::*;
use egui::{
    self, epaint::TextShape, Align2, Color32, FontId, FontFamily, Mesh, Pos2, Rect, Sense, Shape, Stroke, Vec2,
};
use std::f32::consts::PI;

pub const WALL: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1e);
pub const PINK: Color32 = Color32::from_rgb(0xff, 0x2e, 0x88);
pub const ACID: Color32 = Color32::from_rgb(0xb6, 0xff, 0x00);
pub const CAUTION: Color32 = Color32::from_rgb(0xff, 0xd4, 0x00);
pub const CHALK: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xee);
pub const SMOKE: Color32 = Color32::from_rgb(0x8e, 0x8e, 0x94);
pub const INK: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0b);

pub fn marker(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("marker".into()))
}
pub fn block(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("block".into()))
}

/// Tiny deterministic RNG so the wall texture doesn't shimmer between frames.
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub fn draw_wall(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, 0.0, WALL);
    // Cinder-block joints.
    let joint = Color32::from_rgba_unmultiplied(0, 0, 0, 70);
    let (bw, bh) = (120.0, 44.0);
    let mut row = 0;
    let mut y = r.top() + 30.0;
    while y < r.bottom() {
        p.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(2.0_f32, joint));
        let offset = if row % 2 == 0 { 0.0 } else { bw / 2.0 };
        let mut x = r.left() + offset;
        while x < r.right() {
            p.line_segment([Pos2::new(x, y), Pos2::new(x, y + bh)], Stroke::new(2.0_f32, joint));
            x += bw;
        }
        y += bh;
        row += 1;
    }
    // Concrete grit.
    let mut rng = Lcg(0x5eed);
    for _ in 0..1800 {
        let pos = Pos2::new(r.left() + rng.next() * r.width(), r.top() + rng.next() * r.height());
        let light = rng.next() > 0.5;
        let a = (rng.next() * 26.0) as u8 + 6;
        let c = if light {
            Color32::from_rgba_unmultiplied(255, 255, 255, a)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, a + 20)
        };
        let s = 0.6 + rng.next() * 1.6;
        p.rect_filled(Rect::from_center_size(pos, Vec2::splat(s)), 0.0, c);
    }
    // Old faded paint splats.
    let mut rng = Lcg(0xb0b);
    for _ in 0..7 {
        let c = Pos2::new(r.left() + rng.next() * r.width(), r.top() + rng.next() * r.height());
        let col = if rng.next() > 0.5 { PINK } else { ACID };
        for _ in 0..40 {
            let ang = rng.next() * 2.0 * PI;
            let d = rng.next().powf(2.0) * 38.0;
            let dot = c + Vec2::new(ang.cos(), ang.sin()) * d;
            p.circle_filled(dot, 0.6 + rng.next() * 1.4, col.gamma_multiply(0.07));
        }
    }
}

pub fn draw_caution_tape(p: &egui::Painter, r: Rect, text: &str) {
    p.rect_filled(r, 0.0, CAUTION);
    let mut mesh = Mesh::default();
    let stripe = 18.0;
    let mut x = r.left() - r.height();
    let mut k = 0;
    while x < r.right() {
        if k % 2 == 0 {
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(x, r.bottom()), INK);
            mesh.colored_vertex(Pos2::new(x + stripe, r.bottom()), INK);
            mesh.colored_vertex(Pos2::new(x + stripe + r.height(), r.top()), INK);
            mesh.colored_vertex(Pos2::new(x + r.height(), r.top()), INK);
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
        x += stripe;
        k += 1;
    }
    p.add(Shape::mesh(mesh));
    let label = Rect::from_center_size(r.center(), Vec2::new(420.0, r.height()));
    p.rect_filled(label, 0.0, CAUTION);
    p.text(label.center(), Align2::CENTER_CENTER, text, block(15.0), INK);
}

pub fn tilted_text(p: &egui::Painter, pos: Pos2, text: &str, font: FontId, color: Color32, angle: f32) {
    let galley = p.layout_no_wrap(text.to_string(), font, color);
    p.add(Shape::Text(TextShape::new(pos, galley, color).with_angle(angle)));
}

pub fn draw_tag(p: &egui::Painter, at: Pos2, text: &str, sticker_text: &str, sticker_x: f32) {
    let angle = -0.06;
    // Spray halo.
    for (r, a) in [(9.0, 10u8), (6.0, 18), (3.5, 30)] {
        for k in 0..12 {
            let ang = k as f32 / 12.0 * 2.0 * PI;
            let off = Vec2::new(ang.cos(), ang.sin()) * r;
            tilted_text(p, at + off, text, marker(60.0), Color32::from_rgba_unmultiplied(0xff, 0x2e, 0x88, a), angle);
        }
    }
    // Hard black outline, then fill.
    for k in 0..8 {
        let ang = k as f32 / 8.0 * 2.0 * PI;
        tilted_text(p, at + Vec2::new(ang.cos(), ang.sin()) * 2.5, text, marker(60.0), INK, angle);
    }
    tilted_text(p, at, text, marker(60.0), CHALK, angle);

    // Paint drips off the bottom of the letters (follow the tag's tilt).
    let mut rng = Lcg(808);
    for i in 0..9 {
        let dx = 22.0 + i as f32 * 26.0 + rng.next() * 10.0;
        let x = at.x + dx;
        let top = at.y + 72.0 + dx * angle;
        let len = 6.0 + rng.next().powf(1.5) * 26.0;
        let w = 2.0 + rng.next() * 2.0;
        let col = if i % 3 == 1 { PINK } else { CHALK.gamma_multiply(0.85) };
        p.line_segment([Pos2::new(x, top), Pos2::new(x, top + len)], Stroke::new(w, col));
        p.circle_filled(Pos2::new(x, top + len), w * 0.85, col);
    }

    // Sticker next to the tag.
    let sticker = Rect::from_min_size(at + Vec2::new(sticker_x, 38.0), Vec2::new(186.0, 22.0));
    p.rect_filled(sticker.translate(Vec2::new(2.0, 2.0)), 2.0, Color32::from_black_alpha(120));
    p.rect_filled(sticker, 2.0, ACID);
    p.text(sticker.center(), Align2::CENTER_CENTER, sticker_text, block(13.0), INK);
}

pub fn panel(p: &egui::Painter, r: Rect) {
    p.rect_filled(r.translate(Vec2::new(3.0, 4.0)), 6.0, Color32::from_black_alpha(130));
    p.rect_filled(r, 6.0, Color32::from_rgba_unmultiplied(10, 10, 11, 225));
    // Masking tape on the top corners.
    for c in [r.left_top(), r.right_top()] {
        let tape = Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(34.0, 12.0));
        p.rect_filled(tape, 1.0, Color32::from_rgba_unmultiplied(0xe8, 0xe0, 0xc8, 150));
    }
}

pub fn sticker(p: &egui::Painter, at: Pos2, text: &str, bg: Color32, fg: Color32) {
    let galley = p.layout_no_wrap(text.to_string(), block(12.0), fg);
    let r = Rect::from_min_size(at, galley.size() + Vec2::new(12.0, 4.0));
    p.rect_filled(r.translate(Vec2::new(2.0, 2.0)), 2.0, Color32::from_black_alpha(140));
    p.rect_filled(r, 2.0, bg);
    p.galley(at + Vec2::new(6.0, 2.0), galley, fg);
}

pub fn knob_row(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    r: Rect,
    knobs: &[(&str, &FloatParam, Color32)],
    title: &str,
    title_bg: Color32,
) {
    let painter = ui.painter().clone();
    panel(&painter, r);
    let cell_w = r.width() / knobs.len() as f32;
    for (i, (label, param, color)) in knobs.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(r.left() + cell_w * i as f32, r.top()), Vec2::new(cell_w, r.height()));
        knob(ui, setter, cell, label, param, *color);
    }
    sticker(&painter, r.left_top() + Vec2::new(14.0, -10.0), title, title_bg, INK);
}

pub fn knob(ui: &mut egui::Ui, setter: &ParamSetter, cell: Rect, label: &str, param: &FloatParam, color: Color32) {
    let radius = 24.0;
    let center = Pos2::new(cell.center().x, cell.top() + 40.0);
    let hit = Rect::from_center_size(center, Vec2::splat(radius * 2.4));
    let id = ui.scope_id().with(label);
    let resp = ui.interact(hit, id, Sense::click_and_drag());
    // Stepped knobs (LENGTH) move one step per scroll notch.
    let step = param.step_count().map(|n| 1.0 / n.max(1) as f32).unwrap_or(0.01);

    if resp.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
    } else {
        if resp.drag_started() {
            setter.begin_set_parameter(param);
            // Remember where the drag started: adding tiny per-frame moves to a
            // stepped value just snaps back, so accumulate the whole drag instead.
            let start = param.unmodulated_normalized_value();
            ui.memory_mut(|m| m.data.insert_temp(id, (start, 0.0f32)));
        }
        if resp.dragged() {
            // Ctrl (FL's fine-tune key) or Shift = slow, for dialing in.
            let fine = ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift);
            let speed = if fine { 0.0005 } else { 0.005 };
            let d = resp.drag_delta();
            let (start, mut acc) = ui
                .memory(|m| m.data.get_temp::<(f32, f32)>(id))
                .unwrap_or((param.unmodulated_normalized_value(), 0.0));
            acc += (d.x - d.y) * speed;
            ui.memory_mut(|m| m.data.insert_temp(id, (start, acc)));
            setter.set_parameter_normalized(param, (start + acc).clamp(0.0, 1.0));
        }
        if resp.drag_stopped() {
            setter.end_set_parameter(param);
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.events.iter().map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 }).sum::<f32>());
            if scroll != 0.0 {
                let n = (param.unmodulated_normalized_value() + scroll.signum() * step).clamp(0.0, 1.0);
                setter.begin_set_parameter(param);
                setter.set_parameter_normalized(param, n);
                setter.end_set_parameter(param);
            }
        }
    }

    let p = ui.painter();
    let start = 0.75 * PI;
    let sweep = 1.5 * PI;
    let v = param.unmodulated_normalized_value();
    let arc = |from: f32, to: f32, rad: f32| -> Vec<Pos2> {
        let steps = 40;
        (0..=steps)
            .map(|k| {
                let a = from + (to - from) * k as f32 / steps as f32;
                center + Vec2::new(a.cos(), a.sin()) * rad
            })
            .collect()
    };
    p.circle_filled(center + Vec2::new(2.0, 4.0), radius - 4.0, Color32::from_black_alpha(150));
    p.add(Shape::line(arc(start, start + sweep, radius), Stroke::new(5.0_f32, Color32::from_rgb(0x2a, 0x2a, 0x2e))));
    if v > 0.002 {
        p.add(Shape::line(arc(start, start + sweep * v, radius), Stroke::new(11.0_f32, color.gamma_multiply(0.12))));
        p.add(Shape::line(arc(start, start + sweep * v, radius), Stroke::new(5.0_f32, color)));
    }
    // Spray-can cap body.
    let hot = resp.hovered() || resp.dragged();
    p.circle_filled(center, radius - 7.0, Color32::from_rgb(0x0c, 0x0c, 0x0d));
    p.circle_filled(
        center,
        radius - 10.0,
        if hot { Color32::from_rgb(0x30, 0x30, 0x34) } else { Color32::from_rgb(0x24, 0x24, 0x27) },
    );
    p.circle_stroke(center, radius - 10.0, Stroke::new(1.0_f32, Color32::from_white_alpha(18)));
    let a = start + sweep * v;
    let dir = Vec2::new(a.cos(), a.sin());
    p.line_segment([center + dir * 5.0, center + dir * (radius - 12.0)], Stroke::new(3.0_f32, color));
    p.circle_filled(center, 3.5, color);

    // Label on black tape + value.
    let tape = Rect::from_center_size(Pos2::new(center.x, center.y + radius + 13.0), Vec2::new(68.0, 17.0));
    p.rect_filled(tape, 1.0, INK);
    p.text(tape.center(), Align2::CENTER_CENTER, label, block(12.0), CHALK);
    p.text(
        Pos2::new(center.x, tape.bottom() + 11.0),
        Align2::CENTER_CENTER,
        param.normalized_value_to_string(v, true),
        block(15.0),
        color,
    );
}

/// Sticker-style on/off switch.
pub fn bool_toggle(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    at: Pos2,
    param: &BoolParam,
    labels: (&str, &str),
    id: &str,
) {
    let on = param.value();
    let text = if on { labels.0 } else { labels.1 };
    let p = ui.painter();
    let galley = p.layout_no_wrap(text.to_string(), block(12.0), INK);
    let r = Rect::from_min_size(at, galley.size() + Vec2::new(14.0, 4.0));
    let resp = ui.interact(r, ui.scope_id().with(id), Sense::click());
    if resp.clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, !on);
        setter.end_set_parameter(param);
    }
    let bg = if on { CAUTION } else { SMOKE };
    p.rect_filled(r.translate(Vec2::new(2.0, 2.0)), 2.0, Color32::from_black_alpha(140));
    p.rect_filled(r, 2.0, if resp.hovered() { bg.gamma_multiply(1.15) } else { bg });
    p.galley(at + Vec2::new(7.0, 2.0), galley, INK);
}
