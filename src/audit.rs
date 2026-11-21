//! Listening test in numbers: bass notes from silence through the whole rack.
//! Prints, per setting: ONSET (first 5 ms of a note vs. its settled peak, dB),
//! HISS (level in the gaps 80-140 ms after a note ends, dBFS), and CRACKLE
//! (peak of the >9 kHz residue vs. its RMS: clicks give a high crest, dB).
//! `cargo test --release --lib audit -- --nocapture`
use super::*;
use crate::dsp::{Bq, Kind};

const SR: f32 = 48000.0;

pub fn reset_all(p: &ChopParams) {
    for f in [
        &p.input, &p.low_keep, &p.drive, &p.color, &p.phat_mix, &p.depth, &p.time, &p.up, &p.down, &p.ott_low, &p.ott_mid, &p.ott_high,
        &p.buzz_freq, &p.buzz_drive, &p.buzz_mix, &p.vowel, &p.vowel_res, &p.vowel_shift, &p.vowel_mix, &p.chaos_rate, &p.chaos_smooth,
        &p.chaos_vowel, &p.chaos_drive, &p.chaos_buzz, &p.width, &p.spread, &p.mono_below, &p.clip_drive, &p.knee, &p.mix, &p.output,
    ] {
        f.smoothed.reset(f.value());
    }
    crate::audit_extra_reset(p);
}

/// A plugin built from params changed by `f`.
pub fn shop(f: impl FnOnce(&mut ChopParams)) -> ChopShop {
    let mut p = ChopParams::default();
    f(&mut p);
    reset_all(&p);
    let mut cs = ChopShop::default();
    cs.params = Arc::new(p);
    cs.sr = SR;
    cs.osf = oversample::factor_for(cs.params.os.value());
    cs.rebuild();
    cs.byp.set_sr(SR);
    cs.ceil.set_sr(SR);
    cs
}

/// Saw bass, 4 notes, 300 ms on (2 ms attack, 10 ms release) / 200 ms off.
pub fn bass() -> Vec<f32> {
    let mut x = Vec::new();
    let mut ph = 0.0f32;
    for (k, f) in [55.0f32, 73.4, 41.2, 61.7, 55.0, 82.4].iter().enumerate() {
        let _ = k;
        for i in 0..(0.5 * SR) as usize {
            let t = i as f32 / SR;
            let env = if t < 0.002 { t / 0.002 } else if t < 0.3 { 1.0 } else if t < 0.31 { 1.0 - (t - 0.3) / 0.01 } else { 0.0 };
            ph = (ph + f / SR).fract();
            // band-limited-ish saw (sum of 30 harmonics)
            let mut v = 0.0;
            for h in 1..=30 {
                if f * h as f32 > 18000.0 {
                    break;
                }
                v += (std::f32::consts::TAU * ph * h as f32).sin() / h as f32;
            }
            // plus a -75 dBFS noise floor, like any real synth / recording has
            let nz = ((x.len() as u32).wrapping_mul(2654435761) >> 8) as f32 / 8388608.0 - 1.0;
            x.push(0.35 * v * env + 0.00018 * nz * 1.7);
        }
    }
    x
}

pub struct Score {
    pub onset: f32,
    pub hiss: f32,
    pub crackle: f32,
    pub click: f32,
    pub hfpk: f32,
    pub peak: f32,
}

pub fn score(cs: &mut ChopShop, x: &[f32]) -> Score {
    let p = cs.params.clone();
    let mut ch = 0.0;
    let y: Vec<f32> = x.iter().map(|&v| cs.frame(v, v, &p, &mut ch).0).collect();
    let lat = oversample::latency(cs.osf);
    let note = (0.5 * SR) as usize;
    let db = |v: f32| 20.0 * v.max(1e-9).log10();
    let (mut onset, mut hiss) = (-99.0f32, -200.0f32);
    for n in 1..x.len() / note {
        let s = n * note + lat;
        let first = y[s..s + 240].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let settled = y[s + 4800..s + 14000].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        onset = onset.max(db(first) - db(settled));
        let g0 = s + (0.31 * SR) as usize + (0.08 * SR) as usize;
        let g = &y[g0..g0 + (0.06 * SR) as usize];
        let r = (g.iter().map(|v| v * v).sum::<f32>() / g.len() as f32).sqrt();
        hiss = hiss.max(db(r));
    }
    let mut hp = [Bq::default(); 4];
    for h in hp.iter_mut() {
        h.set(Kind::Hp, 9000.0, 0.707, SR);
    }
    // the >9 kHz residue, with 15 ms around every note start / end left out
    let r: Vec<f32> = y.iter().enumerate().map(|(i, &v)| (i, hp.iter_mut().fold(v, |a, h| h.tick(a)))).filter(|(i, _)| {
        let t = (*i as isize - lat as isize).rem_euclid(note as isize) as f32 / SR;
        *i > note && t > 0.015 && !(0.29..0.33).contains(&t)
    }).map(|(_, v)| v).collect();
    let rms = (r.iter().map(|v| v * v).sum::<f32>() / r.len() as f32).sqrt();
    let pk = r.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    // sample-to-sample jumps the music itself doesn't have: 2nd difference peak vs. its RMS
    let d2: Vec<f32> = y.windows(3).skip(note).map(|w| w[2] - 2.0 * w[1] + w[0]).collect();
    let d2r = (d2.iter().map(|v| v * v).sum::<f32>() / d2.len() as f32).sqrt();
    let d2p = d2.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    Score { onset, hiss, crackle: db(pk) - db(rms), click: db(d2p) - db(d2r), hfpk: db(pk), peak: db(y.iter().fold(0.0f32, |m, v| m.max(v.abs()))) }
}

fn only(p: &mut ChopParams) {
    p.phat_on = BoolParam::new("Phat On", false);
    p.ott_on = BoolParam::new("OTT On", false);
    p.wide_on = BoolParam::new("Wide On", false);
    p.clip_on = BoolParam::new("Clip On", false);
}

#[test]
fn audit() {
    let x = bass();
    let cases: Vec<(&str, Box<dyn Fn(&mut ChopParams)>)> = vec![
        ("defaults", Box::new(|_p| {})),
        ("defaults, drive 24", Box::new(|p| p.drive = db_param("Phat Drive", 24.0, 0.0, 36.0))),
        ("defaults, drive 36", Box::new(|p| p.drive = db_param("Phat Drive", 36.0, 0.0, 36.0))),
        ("OTT only, depth 100", Box::new(|p| {
            only(p);
            p.ott_on = BoolParam::new("OTT On", true);
            p.depth = pct("OTT Depth", 1.0);
        })),
        ("PHAT only, drive 30", Box::new(|p| {
            only(p);
            p.phat_on = BoolParam::new("Phat On", true);
            p.drive = db_param("Phat Drive", 30.0, 0.0, 36.0);
        })),
        ("PHAT only, drive 30, 8x", Box::new(|p| {
            only(p);
            p.phat_on = BoolParam::new("Phat On", true);
            p.drive = db_param("Phat Drive", 30.0, 0.0, 36.0);
            p.os = IntParam::new("Oversampling", 3, IntRange::Linear { min: 0, max: 3 });
        })),
        ("PHAT only, drive 30, OFF", Box::new(|p| {
            only(p);
            p.phat_on = BoolParam::new("Phat On", true);
            p.drive = db_param("Phat Drive", 30.0, 0.0, 36.0);
            p.os = IntParam::new("Oversampling", 0, IntRange::Linear { min: 0, max: 3 });
        })),
        ("BUZZ only", Box::new(|p| {
            only(p);
            p.buzz_on = BoolParam::new("Buzz On", true);
        })),
        ("VOWEL only, res 100", Box::new(|p| {
            only(p);
            p.vowel_on = BoolParam::new("Vowel On", true);
            p.vowel_res = pct("Vowel Res", 1.0);
        })),
        ("CHAOS>VOWEL 8 Hz smooth 0", Box::new(|p| {
            only(p);
            p.vowel_on = BoolParam::new("Vowel On", true);
            p.chaos_on = BoolParam::new("Chaos On", true);
            p.chaos_rate = FloatParam::new("Chaos Rate", 8.0, FloatRange::Linear { min: 0.05, max: 30.0 });
            p.chaos_smooth = pct("Chaos Smooth", 0.0);
            p.chaos_vowel = pct("Chaos > Vowel", 1.0);
        })),
        ("CHAOS>VOWEL 8 Hz smooth 100", Box::new(|p| {
            only(p);
            p.vowel_on = BoolParam::new("Vowel On", true);
            p.chaos_on = BoolParam::new("Chaos On", true);
            p.chaos_rate = FloatParam::new("Chaos Rate", 8.0, FloatRange::Linear { min: 0.05, max: 30.0 });
            p.chaos_smooth = pct("Chaos Smooth", 1.0);
            p.chaos_vowel = pct("Chaos > Vowel", 1.0);
        })),
        ("CHAOS>DRIVE 20 Hz smooth 0", Box::new(|p| {
            only(p);
            p.phat_on = BoolParam::new("Phat On", true);
            p.chaos_on = BoolParam::new("Chaos On", true);
            p.chaos_rate = FloatParam::new("Chaos Rate", 20.0, FloatRange::Linear { min: 0.05, max: 30.0 });
            p.chaos_smooth = pct("Chaos Smooth", 0.0);
            p.chaos_drive = pct("Chaos > Drive", 1.0);
        })),
        ("CHAOS>BUZZ 20 Hz smooth 0", Box::new(|p| {
            only(p);
            p.buzz_on = BoolParam::new("Buzz On", true);
            p.chaos_on = BoolParam::new("Chaos On", true);
            p.chaos_rate = FloatParam::new("Chaos Rate", 20.0, FloatRange::Linear { min: 0.05, max: 30.0 });
            p.chaos_smooth = pct("Chaos Smooth", 0.0);
            p.chaos_buzz = pct("Chaos > Buzz", 1.0);
        })),
        ("everything on + chaos all", Box::new(|p| {
            p.buzz_on = BoolParam::new("Buzz On", true);
            p.vowel_on = BoolParam::new("Vowel On", true);
            p.chaos_on = BoolParam::new("Chaos On", true);
            p.chaos_drive = pct("Chaos > Drive", 1.0);
            p.chaos_buzz = pct("Chaos > Buzz", 1.0);
            p.drive = db_param("Phat Drive", 18.0, 0.0, 36.0);
        })),
        ("all modules off (reference)", Box::new(|p| only(p))),
    ];
    println!("{:<30} {:>8} {:>8} {:>9} {:>7}", "", "ONSET", "HISS", "CRACKLE", "PEAK");
    println!("{:<30} {:>8} {:>8} {:>9} {:>7} {:>7}", "", "", "", "", "", "JUMPS");
    for (name, f) in cases {
        let mut cs = shop(|p| f(p));
        let sc = score(&mut cs, &x);
        println!("{:<30} {:>+7.1}  {:>7.1}  {:>8.1}  {:>+6.1} {:>7.1} {:>7.1}", name, sc.onset, sc.hiss, sc.crackle, sc.peak, sc.click, sc.hfpk);
    }
}
