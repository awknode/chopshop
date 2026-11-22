//! Test-only: drives the real UI code with a fake mouse to check knob drags.
use crate::awk_ui::{install_fonts, Look, Skin};
use crate::presets::PresetBar;
use crate::ChopParams;
use nice_plug::context::gui::GuiContextInner;
use nice_plug::prelude::*;
use nice_plug::plugin::PluginState;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use std::sync::Mutex;

struct Mock(Mutex<Vec<String>>);
impl GuiContextInner for Mock {
    fn plugin_api(&self) -> PluginApi { PluginApi::Vst3 }
    fn request_restart(&self) {}
    unsafe fn raw_begin_set_parameter(&self, _p: ParamPtr) { self.0.lock().unwrap().push("begin".into()); }
    unsafe fn raw_set_parameter_normalized(&self, _p: ParamPtr, n: f32) { self.0.lock().unwrap().push(format!("set {n:.3}")); }
    unsafe fn raw_end_set_parameter(&self, _p: ParamPtr) { self.0.lock().unwrap().push("end".into()); }
    fn get_state(&self) -> PluginState { PluginState { version: String::new(), params: Default::default(), fields: Default::default() } }
    fn set_state(&self, _s: PluginState) { self.0.lock().unwrap().push("SET_STATE".into()); }
}

fn drag_knob(with_strip: bool) -> Vec<String> {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let mut skin = Skin::new(Look::Blackface, egui::Color32::WHITE, "TEST", "0", "");
    let params = ChopParams::default();
    let mock = Mock(Mutex::new(Vec::new()));
    let setter = ParamSetter::new(&mock);
    let mut bar = PresetBar::new("DragTest");
    let knob_at = Pos2::new(145.0, 240.0);
    let mut events: Vec<Vec<Event>> = vec![vec![Event::PointerMoved(knob_at)], vec![Event::PointerButton { pos: knob_at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]];
    for k in 1..=6 {
        events.push(vec![Event::PointerMoved(knob_at + Vec2::new(0.0, -10.0 * k as f32))]);
    }
    events.push(vec![Event::PointerButton { pos: knob_at + Vec2::new(0.0, -60.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    events.push(vec![]);
    for (i, ev) in events.into_iter().enumerate() {
        let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 744.0))), time: Some(i as f64 * 0.016), events: ev, ..Default::default() };
        let _ = ctx.run_ui(raw, |ctx| {
            skin.begin(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                if with_strip {
                    let _ = bar.strip(ui, &setter, &params, ui.max_rect(), &skin, &params.mix, &params.bypass);
                }
                skin.knob(ui, &setter, knob_at, 26.0, "DRIVE", &params.drive, true, false, false);
            });
        });
    }
    mock.0.into_inner().unwrap()
}

#[test]
fn knob_drag_reaches_the_host() {
    let without = drag_knob(false);
    let with = drag_knob(true);
    println!("WITHOUT strip: {without:?}");
    println!("WITH strip:    {with:?}");
    assert!(without.iter().any(|s| s.starts_with("set")), "knob alone should drag");
    assert!(with.iter().any(|s| s.starts_with("set")), "knob under the preset strip should drag too");
}

/// Loading a preset must set controls one by one and never call the blocking
/// whole-state restore (that froze FL).
#[test]
fn preset_load_never_blocks() {
    let d = crate::presets::dir("LoadTest");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("Test.awkp"), r#"{"version":"x","params":{"drive":{"f32":20.0},"buzz":{"bool":true},"cseed":{"i32":5}},"fields":{}}"#).unwrap();
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let skin = Skin::new(Look::Machined, egui::Color32::WHITE, "TEST", "0", "");
    let params = ChopParams::default();
    let mock = Mock(Mutex::new(Vec::new()));
    let setter = ParamSetter::new(&mock);
    let mut bar = PresetBar::new("LoadTest");
    // ">" button: right after the name box; click it once (INIT -> Test).
    let mut next_at = Pos2::ZERO;
    for i in 0..4 {
        let ev = match i {
            1 => vec![Event::PointerMoved(next_at)],
            2 => vec![Event::PointerButton { pos: next_at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }],
            3 => vec![Event::PointerButton { pos: next_at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }],
            _ => vec![],
        };
        let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 744.0))), time: Some(i as f64 * 0.016), events: ev, ..Default::default() };
        let _ = ctx.run_ui(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let whole = ui.max_rect();
                let _ = bar.strip(ui, &setter, &params, whole, &skin, &params.mix, &params.bypass);
                // '>' is the last 22 px of the preset box, which ends 12 px left of SAVE (r-378).
                next_at = Pos2::new(whole.right() - 401.0, whole.top() + 22.0);
            });
        });
    }
    let log = mock.0.into_inner().unwrap();
    println!("LOAD LOG: {log:?}");
    assert!(!log.iter().any(|s| s == "SET_STATE"), "must not use the blocking restore");
    assert!(log.iter().filter(|s| s.starts_with("set")).count() >= 3, "should set the preset's controls");
    assert_eq!(bar.current, "Test");
}

/// SAVE AS: typing a name and pressing SAVE writes it to the User folder and
/// favourites it; clicking outside the box cancels without saving.
#[test]
fn save_as_goes_to_user_and_favourites() {
    let plugin = "SaveTest";
    let d = crate::presets::dir(plugin);
    let _ = std::fs::remove_dir_all(&d);
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let mut skin = Skin::new(Look::Blackface, egui::Color32::WHITE, "TEST", "0", "");
    let params = ChopParams::default();
    let mock = Mock(Mutex::new(Vec::new()));
    let setter = ParamSetter::new(&mock);
    let mut bar = PresetBar::new(plugin);
    let w = 960.0f32;
    // SAVE AS chip: r-324 .. r-252, centred on the 44 px header.
    let save_as = Pos2::new(w - 288.0, 22.0);
    let mut frame = |ev: Vec<Event>, i: usize| {
        let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, 744.0))), time: Some(i as f64 * 0.016), events: ev, ..Default::default() };
        let _ = ctx.run_ui(raw, |ctx| {
            skin.begin(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = bar.strip(ui, &setter, &params, ui.max_rect(), &skin, &params.mix, &params.bypass);
            });
        });
    };
    let click = |p: Pos2| vec![Event::PointerMoved(p), Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }];
    let release = |p: Pos2| vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }];
    let mut i = 0;
    let mut step = |ev: Vec<Event>, frame: &mut dyn FnMut(Vec<Event>, usize)| { frame(ev, i); i += 1; };
    step(vec![], &mut frame);
    step(click(save_as), &mut frame);
    step(release(save_as), &mut frame);
    step(vec![], &mut frame);
    step(vec![Event::Text("My Throw".into())], &mut frame);
    step(vec![], &mut frame);
    // Box sits under the preset name: SAVE chip is the first chip in it.
    // Press Enter instead of hunting for pixels.
    step(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], &mut frame);
    step(vec![], &mut frame);
    let f = d.join("User").join("My Throw.awkp");
    assert!(f.exists(), "saved into User: {f:?}");
    let favs = std::fs::read_to_string(d.join("favorites.txt")).unwrap_or_default();
    assert!(favs.lines().any(|l| l == "USER - My Throw"), "favourited: {favs:?}");
    // Open again, type, then click far away: cancels, nothing new saved.
    step(click(save_as), &mut frame);
    step(release(save_as), &mut frame);
    step(vec![Event::Text("X".into())], &mut frame);
    let away = Pos2::new(480.0, 600.0);
    step(click(away), &mut frame);
    step(release(away), &mut frame);
    step(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], &mut frame);
    step(vec![], &mut frame);
    assert!(!d.join("User").join("My ThrowX.awkp").exists(), "click outside cancels");
    let _ = std::fs::remove_dir_all(&d);
}
