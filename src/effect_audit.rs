//! How much each module actually changes the sound (test only).
//! `cargo test --release --lib effect_audit -- --nocapture`
//! CHANGE = level of (output - input) relative to the input, dB. 0 dB = completely different,
//! -6 dB = obvious, -20 dB = subtle, below -40 dB = you won't hear it. LEVEL = output vs input level.
use super::audit::{bass, shop};
use super::*;

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}
fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}
fn off(p: &mut ChopParams) {
    p.phat_on = BoolParam::new("Phat On", false);
    p.ott_on = BoolParam::new("OTT On", false);
    p.wide_on = BoolParam::new("Wide On", false);
    p.clip_on = BoolParam::new("Clip On", false);
    p.buzz_on = BoolParam::new("Buzz On", false);
    p.vowel_on = BoolParam::new("Vowel On", false);
    p.chaos_on = BoolParam::new("Chaos On", false);
    p.os = IntParam::new("Oversampling", 0, IntRange::Linear { min: 0, max: 3 });
}
fn measure(name: &str, f: impl FnOnce(&mut ChopParams)) {
    let x = bass();
    let mut cs = shop(f);
    // like the host on activate: every smoother starts at its param's value
    for (_, ptr, _) in cs.params.param_map() {
        unsafe { ptr._internal_update_smoother(48000.0, true) };
    }
    let mut ch = 0.0;
    let (mut yl, mut yr) = (Vec::new(), Vec::new());
    for &s in &x {
        let (a, b) = cs.frame(s, s * 0.9, &cs.params.clone(), &mut ch);
        yl.push(a);
        yr.push(b);
    }
    let d: Vec<f32> = x.iter().zip(&yl).map(|(a, b)| b - a).collect();
    let side: Vec<f32> = yl.iter().zip(&yr).map(|(a, b)| 0.5 * (a - b)).collect();
    println!("{name:<44} CHANGE {:>7.1} dB   LEVEL {:>+6.1} dB   SIDE {:>7.1} dB", db(rms(&d)) - db(rms(&x)), db(rms(&yl)) - db(rms(&x)), db(rms(&side)) - db(rms(&x)));
}

#[test]
fn effect_audit() {
    measure("everything off (should be ~ -inf)", off);
    measure("defaults (as a new instance)", |p| p.os = IntParam::new("Oversampling", 0, IntRange::Linear { min: 0, max: 3 }));
    measure("PHAT only, defaults", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); });
    measure("PHAT only, drive 24 dB", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); });
    measure("PHAT only, drive 36 dB", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 36.0, 0.0, 36.0); });
    measure("PHAT only, drive 24, buzz 100", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); p.ph_buzz = pct("B", 1.0); });
    measure("PHAT only, drive 24, wide 100", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); p.ph_wide = pct("W", 1.0); });
    measure("PHAT only, drive 24, dry 100", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); p.ph_dry = pct("D", 1.0); });
    measure("OTT only, defaults", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); });
    measure("OTT only, depth 100", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.depth = pct("OTT Depth", 1.0); });
    measure("OTT only, depth 100, up 0", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.depth = pct("OTT Depth", 1.0); p.up = pct("U", 0.0); });
    measure("OTT only, depth 100, down 0", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.depth = pct("OTT Depth", 1.0); p.down = pct("D", 0.0); });
    measure("BUZZ only, defaults", |p| { off(p); p.buzz_on = BoolParam::new("Buzz On", true); });
    measure("BUZZ only, mix 100", |p| { off(p); p.buzz_on = BoolParam::new("Buzz On", true); p.buzz_mix = pct("M", 1.0); });
    measure("VOWEL only, defaults", |p| { off(p); p.vowel_on = BoolParam::new("Vowel On", true); });
    measure("VOWEL only, res 100", |p| { off(p); p.vowel_on = BoolParam::new("Vowel On", true); p.vowel_res = pct("R", 1.0); });
    measure("PHAT d24 + CHAOS>DRIVE 100 @2Hz", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); p.chaos_on = BoolParam::new("Chaos On", true); p.chaos_drive = pct("C", 1.0); });
    measure("PHAT d24, no chaos (compare)", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0); });
    measure("VOWEL + CHAOS>VOWEL 100 @2Hz", |p| { off(p); p.vowel_on = BoolParam::new("Vowel On", true); p.chaos_on = BoolParam::new("Chaos On", true); p.chaos_vowel = pct("C", 1.0); });
    measure("WIDE only, defaults", |p| { off(p); p.wide_on = BoolParam::new("Wide On", true); });
    measure("CLIP only, defaults", |p| { off(p); p.clip_on = BoolParam::new("Clip On", true); });
    measure("CLIP only, drive 12", |p| { off(p); p.clip_on = BoolParam::new("Clip On", true); p.clip_drive = db_param("Clip Drive", 12.0, 0.0, 24.0); });
    measure("PHAT on (defaults) + CHAOS on (defaults)", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); p.chaos_on = BoolParam::new("Chaos On", true); });
    measure("PHAT on (defaults), CHAOS off (compare)", |p| { off(p); p.phat_on = BoolParam::new("Phat On", true); });
    measure("OTT time 0", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.time = pct("T", 0.0); });
    measure("OTT time 100", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.time = pct("T", 1.0); });
    measure("OTT mid OUT +6", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.ott_mid = db_param("M", 6.0, -24.0, 24.0); });
    measure("OTT low OUT +6", |p| { off(p); p.ott_on = BoolParam::new("OTT On", true); p.ott_low = db_param("L", 6.0, -24.0, 24.0); });
}
