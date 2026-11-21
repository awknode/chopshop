//! CHOPSHOP editor, awk studios "Blackface" look.
//! Header: presets, A/B, undo/redo, power (bypass), MIX.
//! Top: INPUT (+ meter) | OTT screen (live spectrum, band split, gain per band) | OUTPUT (+ meter).
//! Bottom: one engraved strip per module in signal order, each with its power LED.
//! Knobs: drag (Ctrl or Shift = fine), scroll to nudge, double-click to reset.

use crate::analyzer::Analyzer;
use crate::awk_ui::*;
use crate::presets::PresetBar;
use crate::{ChopParams, Shared};
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, Pos2, Rect};
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 744;
const ACC: Color32 = rgb(0xff, 0xa6, 0x30);
const XOVER: [f32; 2] = [120.0, 2500.0];

pub(crate) struct St {
    presets: PresetBar,
    skin: Skin,
    an: Analyzer,
}

pub(crate) fn new_st(dir: &'static str) -> St {
    St { presets: PresetBar::new(dir), skin: Skin::new(Look::Blackface, ACC, "CHOPSHOP", env!("CARGO_PKG_VERSION"), ""), an: Analyzer::default() }
}

pub fn create(params: Arc<ChopParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    let state = params.editor_state.clone();
    let st = new_st("ChopShop");
    create_egui_editor(
        state,
        st,
        |ctx, _| install_fonts(ctx),
        move |ctx, setter, st| frame(ctx, setter, st, params.as_ref(), shared.as_ref()),
    )
}

pub(crate) fn frame(ctx: &egui::Context, setter: &ParamSetter, st: &mut St, params: &ChopParams, shared: &Shared) {
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    st.skin.begin(ctx);
    st.an.update(&shared.spec);
    let p = params;
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(rgb(0x1b, 0x1c, 0x1f))).show(ctx, |ui| {
        let whole = ui.max_rect();
        st.skin.background(ui.painter(), whole);
        let full = st.presets.strip(ui, setter, params, whole, &st.skin, &p.mix, &p.bypass);
        let painter = ui.painter().clone();
        let (l, r, t) = (full.left(), full.right(), full.top());
        let foot = Rect::from_min_max(pos2(l, full.bottom() - 30.0), full.right_bottom());
        let skin = &mut st.skin;

        // ---- INPUT
        let inp = Rect::from_min_max(pos2(l + 14.0, t + 12.0), pos2(l + 186.0, t + 262.0));
        skin.panel(&painter, inp, "INPUT", "");
        skin.knob(ui, setter, pos2(inp.left() + 106.0, inp.top() + 88.0), 32.0, "INPUT", &p.input, true, true, false);
        skin.knob(ui, setter, pos2(inp.left() + 106.0, inp.top() + 192.0), 20.0, "LOW KEEP", &p.low_keep, false, false, false);
        let m = &shared.meters;
        skin.meter(&painter, inp.left() + 10.0, inp.top() + 54.0, 172.0, "IN", m.db(0), m.db(1), Side::Right, "in");

        // ---- OUTPUT
        let out = Rect::from_min_max(pos2(r - 186.0, t + 12.0), pos2(r - 14.0, t + 262.0));
        skin.panel(&painter, out, "OUTPUT", "");
        skin.knob(ui, setter, pos2(out.left() + 64.0, out.top() + 88.0), 32.0, "OUTPUT", &p.output, true, false, false);
        skin.chip_param(ui, setter, Rect::from_min_size(pos2(out.left() + 12.0, out.top() + 196.0), vec2(104.0, 20.0)), &p.match_level, ("MATCH LEVEL", "MATCH LEVEL"));
        let lat = crate::oversample::latency(crate::oversample::factor_for(p.os.value()));
        skin.small(&painter, pos2(out.left() + 64.0, out.top() + 232.0), Align2::CENTER_CENTER, format!("LATENCY {lat} smp"), skin.text_faint());
        skin.meter(&painter, out.right() - 24.0, out.top() + 54.0, 172.0, "OUT", m.db(2), m.db(3), Side::Left, "out");

        // ---- OTT screen
        let scr = Rect::from_min_max(pos2(inp.right() + 18.0, t + 12.0), pos2(out.left() - 18.0, t + 262.0));
        draw_screen(ui, setter, &painter, skin, scr, p, shared, &st.an);

        // ---- modules
        let top = t + 276.0;
        let bot = foot.top() - 10.0;
        // PHAT is two units wide (8 knobs, phat-rack spec); the rest one unit each
        let sw = (r - l - 28.0 - 7.0 * 8.0) / 8.0;
        let strip = |i: usize| {
            let x0 = if i == 0 { l + 14.0 } else { l + 14.0 + (i as f32 + 1.0) * (sw + 8.0) };
            let w = if i == 0 { 2.0 * sw + 8.0 } else { sw };
            Rect::from_min_max(pos2(x0, top), pos2(x0 + w, bot))
        };
        type Mk<'a> = (&'a str, &'a FloatParam, bool);
        let mods: [(&str, &BoolParam, Mk, Vec<Mk>); 7] = [
            ("PHAT", &p.phat_on, ("DRIVE", &p.drive, false), vec![("COLOR", &p.color, true), ("MIX", &p.phat_mix, false)]),
            (
                "OTT",
                &p.ott_on,
                ("DEPTH", &p.depth, false),
                vec![("TIME", &p.time, false), ("UP", &p.up, false), ("DOWN", &p.down, false), ("LOWS", &p.ott_amt_low, false), ("MIDS", &p.ott_amt_mid, false), ("HIGHS", &p.ott_amt_high, false)],
            ),
            ("BUZZ", &p.buzz_on, ("FREQ", &p.buzz_freq, false), vec![("FUZZ", &p.buzz_drive, false), ("MIX", &p.buzz_mix, false)]),
            ("VOWEL", &p.vowel_on, ("VOWEL", &p.vowel, false), vec![("RES", &p.vowel_res, false), ("SHIFT", &p.vowel_shift, true), ("MIX", &p.vowel_mix, false)]),
            ("CHAOS", &p.chaos_on, ("RATE", &p.chaos_rate, false), vec![("GLIDE", &p.chaos_smooth, false), ("> VOWEL", &p.chaos_vowel, false), ("> DRIVE", &p.chaos_drive, false), ("> BUZZ", &p.chaos_buzz, false)]),
            ("WIDE", &p.wide_on, ("WIDTH", &p.width, false), vec![("SPREAD", &p.spread, false), ("MONO <", &p.mono_below, false)]),
            ("CLIP", &p.clip_on, ("DRIVE", &p.clip_drive, false), vec![("KNEE", &p.knee, false)]),
        ];
        for (i, (title, on_p, big, small)) in mods.iter().enumerate() {
            let s = strip(i);
            if i > 0 {
                skin.divider(&painter, s.left() - 5.0, s.top() + 4.0, s.bottom() - 4.0);
            }
            let on = on_p.value();
            txt(&painter, pos2(s.left() + 10.0, s.top() + 20.0), Align2::LEFT_BOTTOM, *title, labb(15.0), rgb(0xed, 0xed, 0xee), 3.0);
            skin.power(ui, setter, Rect::from_center_size(pos2(s.right() - 16.0, s.top() + 13.0), vec2(16.0, 16.0)), on_p);
            let cx = s.center().x;
            if *title == "PHAT" {
                phat_strip(ui, setter, skin, s, p);
                continue;
            }
            let _ = small;
            skin.knob(ui, setter, pos2(cx, s.top() + 76.0), 26.0, big.0, big.1, true, big.2, !on);
            for (k, (lab_s, prm, bip)) in small.iter().enumerate() {
                let row = k / 2;
                let last_single = k == small.len() - 1 && small.len() % 2 == 1;
                let x = if last_single { cx } else if k % 2 == 0 { cx - 30.0 } else { cx + 30.0 };
                let y = s.top() + 186.0 + row as f32 * 67.0;
                skin.knob(ui, setter, pos2(x, y), 15.0, lab_s, *prm, false, *bip, !on);
            }
            if *title == "CHAOS" {
                chaos_extras(ui, setter, skin, s, p, shared);
            }
        }

        // ---- footer
        skin.footer(&painter, foot, 0.0);
        skin.os_selector(ui, setter, pos2(foot.left() + 16.0, foot.center().y - 8.5), &p.os, 0.9);
        skin.safe_chip(ui, setter, Rect::from_center_size(pos2(foot.right() - 250.0, foot.center().y), vec2(112.0, 18.0)), &p.safe, shared.meters.db(4));
    });
}

/// PHAT, to the phat-rack spec: STRONK / RIDDIM / OFF / INIT, then
/// PHAT, DRY, WIDE, BUZZ over DISPERSER, DISP FREQ, SHIFT, SHIFT X-FREQ.
fn phat_strip(ui: &mut egui::Ui, setter: &ParamSetter, skin: &Skin, s: Rect, p: &ChopParams) {
    let on = p.phat_on.value();
    let mode = p.ph_mode.value().clamp(0, 3) as usize;
    let cw = (s.width() - 20.0 - 3.0 * 4.0) / 4.0;
    for (i, name) in crate::PHAT_MODES.iter().enumerate() {
        let cr = Rect::from_min_size(pos2(s.left() + 10.0 + i as f32 * (cw + 4.0), s.top() + 34.0), vec2(cw, 18.0));
        let hint = ["Heavy: drive, width, some disperser and shift", "Riddim: lots of disperser and buzz, mono-ish", "PHAT off", "Back to the start (light drive, everything else off)"][i];
        if skin.chip(ui, cr, name, mode == i && (on || i == 2), ("phmode", i)).on_hover_text(hint).clicked() {
            apply_phat_mode(setter, p, i);
        }
    }
    let row = [("PHAT", &p.drive), ("DRY", &p.ph_dry), ("WIDE", &p.ph_wide), ("BUZZ", &p.ph_buzz)];
    let row2 = [("DISP", &p.ph_disp), ("DISP HZ", &p.ph_dfreq), ("SHIFT", &p.ph_shift), ("X-FREQ", &p.ph_xf)];
    let step = (s.width() - 12.0) / 4.0;
    for (r, list) in [row, row2].iter().enumerate() {
        for (k, (lab_s, prm)) in list.iter().enumerate() {
            let x = s.left() + 6.0 + step * (k as f32 + 0.5);
            let y = s.top() + 104.0 + r as f32 * 92.0;
            skin.knob(ui, setter, pos2(x, y), 16.0, lab_s, *prm, false, false, !on);
        }
    }
}

fn apply_phat_mode(setter: &ParamSetter, p: &ChopParams, mode: usize) {
    let set_f = |prm: &FloatParam, v: f32| {
        setter.begin_set_parameter(prm);
        setter.set_parameter(prm, v);
        setter.end_set_parameter(prm);
    };
    setter.begin_set_parameter(&p.ph_mode);
    setter.set_parameter(&p.ph_mode, mode as i32);
    setter.end_set_parameter(&p.ph_mode);
    setter.begin_set_parameter(&p.phat_on);
    setter.set_parameter(&p.phat_on, mode != 2);
    setter.end_set_parameter(&p.phat_on);
    // (drive dB, dry, wide, buzz, disperser, disp freq, shift, x-freq)
    let v = match mode {
        0 => (18.0, 0.2, 0.35, 0.3, 0.4, 180.0, 0.25, 300.0),
        1 => (24.0, 0.0, 0.1, 0.6, 0.75, 120.0, 0.08, 400.0),
        2 => return,
        _ => (6.0, 0.0, 0.0, 0.0, 0.0, 250.0, 0.0, 250.0),
    };
    set_f(&p.drive, v.0);
    set_f(&p.ph_dry, v.1);
    set_f(&p.ph_wide, v.2);
    set_f(&p.ph_buzz, v.3);
    set_f(&p.ph_disp, v.4);
    set_f(&p.ph_dfreq, v.5);
    set_f(&p.ph_shift, v.6);
    set_f(&p.ph_xf, v.7);
    set_f(&p.phat_mix, 1.0);
    set_f(&p.color, 0.0);
}

/// OTT screen: live output spectrum, the two crossovers, and each band's gain.
fn draw_screen(ui: &mut egui::Ui, setter: &ParamSetter, painter: &egui::Painter, skin: &Skin, scr: Rect, p: &ChopParams, shared: &Shared, an: &Analyzer) {
    let (sp, pal) = skin.screen(painter, scr);
    let (gl, gr_, top, base) = (scr.left() + 40.0, scr.right() - 14.0, scr.top() + 32.0, scr.bottom() - 24.0);
    let px = |u: f32| gl + u * (gr_ - gl);
    let py = |d: f32| base - (d.clamp(-60.0, 0.0) + 60.0) / 60.0 * (base - top);
    skin.db_axis(&sp, &pal, gl, gr_, py, &[0.0, -12.0, -24.0, -36.0, -48.0, -60.0], gl - 6.0);
    skin.freq_axis(&sp, &pal, px, top, base, base + 4.0);
    let pts: Vec<Pos2> = (0..=200).map(|i| {
        let u = i as f32 / 200.0;
        pos2(px(u), py(an.at(u)))
    }).collect();
    fill_under(&sp, &pts, base, pal.fill(0.14));
    polyline(&sp, pts, pal.trace, 1.6, pal.glow);
    let ott_on = p.ott_on.value();
    let edges = [0.0, fu(XOVER[0]), fu(XOVER[1]), 1.0];
    for e in &edges[1..3] {
        dashed(&sp, pos2(px(*e), top), pos2(px(*e), base), pal.fill(0.45), 1.0, 3.0, 4.0);
    }
    let names = [("LOW", "20-120 Hz"), ("MID", "120 Hz-2.5 kHz"), ("HIGH", "2.5-20 kHz")];
    let gains = [&p.ott_low, &p.ott_mid, &p.ott_high];
    let amts = [&p.ott_amt_low, &p.ott_amt_mid, &p.ott_amt_high];
    for (i, (n, range)) in names.iter().enumerate() {
        let cx = (px(edges[i]) + px(edges[i + 1])) / 2.0;
        let g = f32::from_bits(shared.ott_gr[i].load(Ordering::Relaxed));
        let box_r = Rect::from_center_size(pos2(cx, top + 24.0), vec2(108.0, 42.0));
        // drag a band's box up / down = that band's output gain (double-click resets)
        let gp = gains[i];
        let resp = ui.interact(box_r, ui.id().with(("bandgain", i)), egui::Sense::click_and_drag()).on_hover_text("Drag up / down: this band's output gain. Double-click: 0 dB");
        if resp.double_clicked() {
            setter.begin_set_parameter(gp);
            setter.set_parameter(gp, 0.0);
            setter.end_set_parameter(gp);
        } else {
            if resp.drag_started() {
                setter.begin_set_parameter(gp);
            }
            if resp.dragged() {
                let fine = ui.input(|i| i.modifiers.shift || i.modifiers.ctrl);
                let d = -resp.drag_delta().y * if fine { 0.02 } else { 0.1 };
                setter.set_parameter(gp, (gp.value() + d).clamp(-12.0, 12.0));
            }
            if resp.drag_stopped() {
                setter.end_set_parameter(gp);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        sp.rect_filled(box_r, 2.0, Color32::from_black_alpha(if hot { 200 } else { 160 }));
        if hot {
            sp.rect_stroke(box_r, 2.0, egui::Stroke::new(1.0, pal.fill(0.6)), egui::StrokeKind::Inside);
        }
        let a = amts[i].value();
        txt(&sp, pos2(cx, box_r.top() + 3.0), Align2::CENTER_TOP, format!("{n}  {range}"), mono(9.0), pal.axis, 0.3);
        let s = if !ott_on { "OTT OFF".to_string() } else if a < 0.005 { "BYPASSED".to_string() } else { format!("GAIN {g:+.1} dB") };
        txt(&sp, pos2(cx, box_r.top() + 21.0), Align2::CENTER_CENTER, s, mono(11.0), if a < 0.005 { pal.axis } else { pal.txt }, 0.0);
        txt(&sp, pos2(cx, box_r.bottom() - 2.0), Align2::CENTER_BOTTOM, format!("OUT {:+.1} dB", gp.value()), mono(9.0), if gp.value().abs() > 0.05 { pal.trace } else { pal.axis }, 0.0);
    }
    let sr = f32::from_bits(shared.spec.sr.load(Ordering::Relaxed));
    txt(&sp, pos2(scr.left() + 12.0, scr.top() + 6.0), Align2::LEFT_TOP, format!("OTT · 3 BAND · DEPTH {:.0}%", p.depth.value() * 100.0), mono(10.0), pal.trace, 1.5);
    txt(&sp, pos2(scr.right() - 12.0, scr.top() + 6.0), Align2::RIGHT_TOP, format!("OUTPUT · FFT 4096 · {:.1} kHz", sr / 1000.0), mono(10.0), pal.fill(0.6), 1.0);
    txt(&sp, pos2(gl - 6.0, top - 12.0), Align2::RIGHT_CENTER, "dBFS", mono(8.0), pal.axis, 0.0);
    skin.screen_end(painter, scr);
}

/// CHAOS: live value + REROLL (new random pattern, shows the seed).
fn chaos_extras(ui: &mut egui::Ui, setter: &ParamSetter, skin: &Skin, s: Rect, p: &ChopParams, shared: &Shared) {
    let painter = ui.painter().clone();
    let y = s.top() + 186.0 + 2.0 * 67.0 - 6.0;
    let bar = Rect::from_center_size(pos2(s.center().x, y), vec2(s.width() - 28.0, 8.0));
    painter.rect_filled(bar, 2.0, Color32::from_black_alpha(170));
    line(&painter, pos2(bar.center().x, bar.top() - 2.0), pos2(bar.center().x, bar.bottom() + 2.0), skin.text_faint(), 1.0);
    if p.chaos_on.value() {
        let cv = f32::from_bits(shared.chaos.load(Ordering::Relaxed)).clamp(-1.0, 1.0);
        let c = pos2(bar.center().x + cv * bar.width() * 0.5, bar.center().y);
        painter.circle_filled(c, 7.0, al(skin.acc, 0.2));
        painter.circle_filled(c, 4.0, skin.acc);
    }
    let rr = Rect::from_center_size(pos2(s.center().x, y + 26.0), vec2(s.width() - 28.0, 20.0));
    if skin.chip(ui, rr, &format!("REROLL  #{}", p.chaos_seed.value()), false, "reroll").clicked() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7);
        let mut sd = (t % 999) as i32 + 1;
        if sd == p.chaos_seed.value() {
            sd = sd % 999 + 1;
        }
        setter.begin_set_parameter(&p.chaos_seed);
        setter.set_parameter(&p.chaos_seed, sd);
        setter.end_set_parameter(&p.chaos_seed);
    }
}
