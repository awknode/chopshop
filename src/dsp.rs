//! CHOPSHOP building blocks. Everything is cheap per-sample DSP: biquads,
//! envelope followers and waveshapers. No FFTs, no latency.

use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Default)]
pub struct Bq {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

pub enum Kind {
    Lp,
    Hp,
    /// Band-pass, 0 dB at the centre.
    Bp,
    Ap,
}

impl Bq {
    pub fn set(&mut self, kind: Kind, f: f32, q: f32, sr: f32) {
        let w = TAU * f.clamp(10.0, sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let a = s / (2.0 * q);
        let (b0, b1, b2) = match kind {
            Kind::Lp => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
            Kind::Hp => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
            Kind::Bp => (a, 0.0, -a),
            Kind::Ap => (1.0 - a, -2.0 * c, 1.0 + a),
        };
        let a0 = 1.0 + a;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - a) / a0;
    }
    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
    pub fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

const BW: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Trapezoidal state-variable filter: stays clean while its frequency / Q move
/// every few samples (a biquad clicks and crackles when modulated like that).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: f32,
    ic2: f32,
}

impl Svf {
    pub fn set(&mut self, f: f32, q: f32, sr: f32) {
        let g = (PI * f.clamp(10.0, sr * 0.45) / sr).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// (low, band normalised to 0 dB at the centre, high)
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, self.k * v1, x - self.k * v1 - v2)
    }
}

/// Linkwitz-Riley 4th-order split: (low, high), low + high is all-pass flat.
#[derive(Clone, Copy, Default)]
pub struct Lr4 {
    lp: [Bq; 2],
    hp: [Bq; 2],
}

impl Lr4 {
    pub fn set(&mut self, f: f32, sr: f32) {
        for b in self.lp.iter_mut() {
            b.set(Kind::Lp, f, BW, sr);
        }
        for b in self.hp.iter_mut() {
            b.set(Kind::Hp, f, BW, sr);
        }
    }
    #[inline]
    pub fn split(&mut self, x: f32) -> (f32, f32) {
        let l = self.lp[0].tick(x);
        let l = self.lp[1].tick(l);
        let h = self.hp[0].tick(x);
        let h = self.hp[1].tick(h);
        (l, h)
    }
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db * 0.05)
}

#[inline]
pub fn coef(ms: f32, sr: f32) -> f32 {
    (-1.0 / (ms.max(0.01) * 0.001 * sr)).exp()
}

// ---------------------------------------------------------------- PHAT
/// Thickener: tilt EQ into an asymmetric tube-ish saturator, DC-blocked.
/// Drive pushes harmonics + density; COLOR tilts dark <-> bright first.
#[derive(Clone, Copy, Default)]
pub struct Phat {
    lp: f32,
    dc_x: f32,
    dc_y: f32,
}

impl Phat {
    #[inline]
    pub fn tick(&mut self, x: f32, drive_db: f32, color: f32, sr: f32) -> f32 {
        // Tilt around ~700 Hz.
        let a = 1.0 - (-TAU * 700.0 / sr).exp();
        self.lp += (x - self.lp) * a;
        let hi = x - self.lp;
        let t = db_to_gain(color * 6.0);
        let x = self.lp / t + hi * t;
        let g = db_to_gain(drive_db);
        // Asymmetric: even harmonics (warmth) + odd (bite).
        let v = g * x;
        let y = (v + 0.25).tanh() - 0.25f32.tanh();
        // Rough level match so DRIVE adds density, not just volume.
        let y = y / (1.0 + 0.35 * (g - 1.0)).sqrt();
        // DC blocker, ~8 Hz at any rate: the asymmetry's DC dies in ~20 ms after a
        // note ends instead of leaving a slow thump for OTT to lift.
        let r = 1.0 - TAU * 8.0 / sr;
        let out = y - self.dc_x + r * self.dc_y;
        self.dc_x = y;
        self.dc_y = out;
        out
    }
}

// ---------------------------------------------------------------- PHAT extras
/// The phat-rack extras, per channel: BUZZ (high grit layered on top), DISPERSER
/// (up to 16 allpasses at one frequency: the phase smear / "punch"), SHIFT (a
/// frequency shifter above X-FREQ, so the low end stays put).
#[derive(Clone, Copy, Default)]
struct Ap2s {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}
#[derive(Clone, Copy, Default)]
struct Hs {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}
impl Hs {
    #[inline]
    fn tick(&mut self, x: f32, c: f32) -> f32 {
        let y = c * (x + self.y2) - self.x2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}
// Niemitalo's 4+4 allpass Hilbert pair (squared coefficients)
const HIL_A: [f32; 4] = [0.479_400_9, 0.876_218_5, 0.976_597_6, 0.997_499_3];
const HIL_B: [f32; 4] = [0.161_758_5, 0.733_028_9, 0.945_349_7, 0.990_599_2];
pub const DISP_N: usize = 16;

#[derive(Clone, Copy, Default)]
pub struct PhatPlus {
    bz_lp: f32,
    ap: [Ap2s; DISP_N],
    ap_c: (f32, f32),
    ap_key: f32,
    ha: [Hs; 4],
    hb: [Hs; 4],
    hd: f32,
    xo: f32,
    ph: f32,
}

impl PhatPlus {
    /// `buzz`, `disp` 0..1; `dfreq` Hz; `shift_hz` signed (L up, R down); `xf` Hz.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn tick(&mut self, x: f32, buzz: f32, disp: f32, dfreq: f32, shift_hz: f32, xf: f32, sr: f32) -> f32 {
        let mut y = x;
        // BUZZ: hard-ish grit, high-passed so it rides on top
        if buzz > 0.001 {
            let g = (x * 5.0).tanh() * 0.5;
            let a = 1.0 - (-TAU * 900.0 / sr).exp();
            self.bz_lp += (g - self.bz_lp) * a;
            y += (g - self.bz_lp) * buzz * 0.8;
        }
        // DISPERSER: chain of 2nd-order allpasses, the amount engages more of them
        if disp > 0.001 {
            if (dfreq - self.ap_key).abs() > 0.5 {
                self.ap_key = dfreq;
                let w = TAU * dfreq.clamp(20.0, sr * 0.45) / sr;
                let alpha = w.sin() / (2.0 * 0.6);
                let a0 = 1.0 + alpha;
                self.ap_c = (-2.0 * w.cos() / a0, (1.0 - alpha) / a0);
            }
            let (a1, a2) = self.ap_c;
            let n = disp * DISP_N as f32;
            let full = n.floor() as usize;
            let frac = n - full as f32;
            let mut v = y;
            let mut prev = v;
            for k in 0..DISP_N.min(full + 1) {
                let s = &mut self.ap[k];
                let o = a2 * v + a1 * s.x1 + s.x2 - a1 * s.y1 - a2 * s.y2;
                s.x2 = s.x1;
                s.x1 = v;
                s.y2 = s.y1;
                s.y1 = o;
                prev = v;
                v = o;
            }
            y = if full >= DISP_N { v } else { prev + (v - prev) * frac };
        }
        // SHIFT: frequency-shift what's above X-FREQ
        if shift_hz.abs() > 0.01 {
            let a = 1.0 - (-TAU * xf / sr).exp();
            self.xo += (y - self.xo) * a;
            let hi = y - self.xo;
            let mut i = hi;
            for (k, s) in self.ha.iter_mut().enumerate() {
                i = s.tick(i, HIL_A[k]);
            }
            let id = self.hd;
            self.hd = i;
            let mut q = hi;
            for (k, s) in self.hb.iter_mut().enumerate() {
                q = s.tick(q, HIL_B[k]);
            }
            self.ph = (self.ph + shift_hz / sr).rem_euclid(1.0);
            let (sn, cs) = (TAU * self.ph).sin_cos();
            y = self.xo + id * cs + q * sn;
        }
        y
    }
}

// ---------------------------------------------------------------- OTT
/// 3-band upward + downward compressor (the OTT idea), stereo-linked per band.
pub struct Ott {
    x1: [Lr4; 2],
    x2: [Lr4; 2],
    env: [f32; 3],
    /// smoothed gain per band (dB): falls fast, rises at the release speed, so
    /// a note out of silence never gets the full upward boost on its first ms
    gs: [f32; 3],
    pub gr_db: [f32; 3],
    k: [[f32; 2]; 3],
    k_time: f32,
}

/// Upward lift fades out below this (dBFS): the noise floor, reverb dust and
/// silence between notes stay where they are instead of turning into hiss.
const OTT_FLOOR: f32 = -64.0;

/// Thresholds per band (low, mid, high): (downward above, upward below).
const OTT_DOWN: [f32; 3] = [-28.0, -30.0, -32.0];
const OTT_UP: [f32; 3] = [-40.0, -41.0, -42.0];

impl Ott {
    pub fn new(sr: f32) -> Self {
        let mut o = Self { x1: [Lr4::default(); 2], x2: [Lr4::default(); 2], env: [0.0; 3], gs: [0.0; 3], gr_db: [0.0; 3], k: [[0.0; 2]; 3], k_time: -1.0 };
        for c in 0..2 {
            o.x1[c].set(120.0, sr);
            o.x2[c].set(2500.0, sr);
        }
        o
    }
    /// depth 0..1 (mix), time 0..1 (speed), up / down 0..1 amounts, band gains in
    /// dB, and how much of the OTT each band gets (0..1).
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, depth: f32, time: f32, up: f32, down: f32, band_db: [f32; 3], amt: [f32; 3], sr: f32) -> (f32, f32) {
        let (ll, lr) = self.x1[0].split(l);
        let (ml, hl) = self.x2[0].split(lr);
        let (rl, rr) = self.x1[1].split(r);
        let (mr, hr) = self.x2[1].split(rr);
        let bands = [(ll, rl), (ml, mr), (hl, hr)];
        if (time - self.k_time).abs() > 1e-4 {
            let scale = 0.1 + 2.9 * time * time;
            for b in 0..3 {
                // Lows slower, highs faster.
                self.k[b] = [coef([8.0, 4.0, 2.0][b] * scale, sr), coef([180.0, 120.0, 80.0][b] * scale, sr)];
            }
            self.k_time = time;
        }
        let mut out = (0.0, 0.0);
        for b in 0..3 {
            let (a, c) = bands[b];
            let lvl = a.abs().max(c.abs());
            let [ka, kr] = self.k[b];
            let k = if lvl > self.env[b] { ka } else { kr };
            self.env[b] = lvl + (self.env[b] - lvl) * k;
            let db = 20.0 * (self.env[b] + 1e-9).log10();
            let mut g = 0.0;
            if db > OTT_DOWN[b] {
                g -= (db - OTT_DOWN[b]) * (1.0 - 1.0 / 66.0) * down;
            } else if db < OTT_UP[b] {
                let lift = ((OTT_UP[b] - db) * (1.0 - 1.0 / 4.17) * up).min(24.0);
                g += lift * ((db - OTT_FLOOR) / 12.0).clamp(0.0, 1.0);
            }
            // gain down: at the attack speed; gain up: at the release speed
            let kg = if g < self.gs[b] { ka } else { kr };
            self.gs[b] = g + (self.gs[b] - g) * kg;
            let g = self.gs[b];
            self.gr_db[b] = g;
            // OTT's own makeup so DEPTH 100% isn't a huge level drop.
            let wet = db_to_gain(g + 10.0 + band_db[b]);
            let gain = 1.0 + depth * amt[b] * (wet - 1.0);
            out.0 += a * gain;
            out.1 += c * gain;
        }
        out
    }
}

// ---------------------------------------------------------------- BUZZ
/// Fuzz on the top end only: highs above FREQ go through a wavefolder.
#[derive(Clone, Copy, Default)]
pub struct Buzz {
    hp: [Svf; 2],
}

impl Buzz {
    pub fn set(&mut self, f: f32, sr: f32) {
        for b in self.hp.iter_mut() {
            b.set(f, BW, sr);
        }
    }
    #[inline]
    pub fn tick(&mut self, x: f32, drive_db: f32, mix: f32) -> f32 {
        let hb = self.hp[0].tick(x).2;
        let hb = self.hp[1].tick(hb).2;
        let g = db_to_gain(drive_db);
        let folded = (g * hb * 1.5).sin() / (1.0 + 0.12 * g);
        x + (folded - hb) * mix
    }
}

// ---------------------------------------------------------------- VOWEL
/// Formant filter: morphs A > E > I > O > U (3 formants each).
pub const VOWELS: [&str; 5] = ["A", "E", "I", "O", "U"];
const FORMANTS: [[f32; 3]; 5] = [
    [800.0, 1150.0, 2900.0],
    [400.0, 1700.0, 2600.0],
    [300.0, 2200.0, 3000.0],
    [450.0, 800.0, 2830.0],
    [325.0, 700.0, 2530.0],
];
const FORMANT_DB: [[f32; 3]; 5] = [[0.0, -4.0, -18.0], [0.0, -10.0, -12.0], [0.0, -12.0, -20.0], [0.0, -9.0, -16.0], [0.0, -12.0, -20.0]];

#[derive(Clone, Copy, Default)]
pub struct Vowel {
    bp: [Svf; 3],
    g: [f32; 3],
}

impl Vowel {
    /// `pos` 0..1 across A..U, `res` 0..1, `shift` -1..1 (octave down/up).
    pub fn set(&mut self, pos: f32, res: f32, shift: f32, sr: f32) {
        let p = pos.clamp(0.0, 1.0) * 4.0;
        let i = (p as usize).min(3);
        let t = p - i as f32;
        let q = 3.0 + res * 17.0;
        let sh = 2f32.powf(shift);
        for k in 0..3 {
            let f = FORMANTS[i][k].ln() * (1.0 - t) + FORMANTS[i + 1][k].ln() * t;
            self.bp[k].set(f.exp() * sh, q, sr);
            self.g[k] = db_to_gain(FORMANT_DB[i][k] * (1.0 - t) + FORMANT_DB[i + 1][k] * t);
        }
    }
    #[inline]
    pub fn tick(&mut self, x: f32, res: f32) -> f32 {
        let mut y = 0.0;
        for k in 0..3 {
            y += self.bp[k].tick(x).1 * self.g[k];
        }
        // Narrow bands lose level; make it back.
        y * (1.6 + res * 2.4)
    }
}

// ---------------------------------------------------------------- CHAOS
/// Smooth random LFO (cosine-glided random targets), repeatable from a seed.
#[derive(Clone, Copy)]
pub struct Chaos {
    rng: u32,
    from: f32,
    to: f32,
    t: f32,
    pub value: f32,
}

impl Chaos {
    pub fn new(seed: u32) -> Self {
        let mut c = Self { rng: 0, from: 0.0, to: 0.0, t: 0.0, value: 0.0 };
        c.reseed(seed);
        c
    }
    pub fn reseed(&mut self, seed: u32) {
        self.rng = seed.wrapping_mul(2654435761).wrapping_add(0x9E37_79B9) | 1;
        self.from = 0.0;
        self.to = self.next();
        self.t = 0.0;
    }
    fn next(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    /// `rate` Hz, `smooth` 0 = steppy (sample & hold), 1 = gliding.
    #[inline]
    pub fn tick(&mut self, rate: f32, smooth: f32, sr: f32) -> f32 {
        self.t += rate / sr;
        if self.t >= 1.0 {
            self.t -= 1.0;
            self.from = self.to;
            self.to = self.next();
        }
        let glide = 0.5 - 0.5 * (PI * self.t).cos();
        let step = if self.t < 0.02 { glide * 50.0 * self.t } else { 1.0 };
        let shape = smooth * glide + (1.0 - smooth) * step.min(1.0);
        self.value = self.from + (self.to - self.from) * shape;
        self.value
    }
}

// ---------------------------------------------------------------- WIDE
/// Width + SPREAD (makes stereo from mono with decorrelating all-passes),
/// with everything below MONO kept dead-centre (safe for subs and clubs).
pub struct Wide {
    side_hp: [Bq; 2],
    ap: [[Bq; 4]; 2],
}

const AP_L: [f32; 4] = [190.0, 640.0, 1900.0, 5200.0];
const AP_R: [f32; 4] = [260.0, 870.0, 2600.0, 7400.0];

impl Wide {
    pub fn new(sr: f32) -> Self {
        let mut w = Self { side_hp: [Bq::default(); 2], ap: [[Bq::default(); 4]; 2] };
        for k in 0..4 {
            w.ap[0][k].set(Kind::Ap, AP_L[k], 0.7, sr);
            w.ap[1][k].set(Kind::Ap, AP_R[k], 0.7, sr);
        }
        w
    }
    pub fn set(&mut self, mono_below: f32, sr: f32) {
        for b in self.side_hp.iter_mut() {
            b.set(Kind::Hp, mono_below, BW, sr);
        }
    }
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, width: f32, spread: f32) -> (f32, f32) {
        let m = 0.5 * (l + r);
        let s = 0.5 * (l - r) * width;
        let mut a = m;
        let mut b = m;
        for k in 0..4 {
            a = self.ap[0][k].tick(a);
            b = self.ap[1][k].tick(b);
        }
        let s = s + 0.5 * (a - b) * spread;
        let s = self.side_hp[0].tick(s);
        let s = self.side_hp[1].tick(s);
        (m + s, m - s)
    }
}

// ---------------------------------------------------------------- CLIP
/// Soft-knee clipper, ceiling 0 dBFS.
#[inline]
pub fn clip(x: f32, knee: f32) -> f32 {
    let k = knee.clamp(0.0, 0.9);
    let t = 1.0 - k;
    let a = x.abs();
    if a <= t {
        x
    } else {
        x.signum() * (t + k * ((a - t) / k.max(1e-6)).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48000.0;

    fn tone(f: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (TAU * f * i as f32 / SR).sin()).collect()
    }
    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn ott_lifts_quiet_and_squashes_loud() {
        let n = 48000;
        for (amp, expect_up) in [(0.003f32, true), (0.9, false)] {
            let mut o = Ott::new(SR);
            let x = tone(800.0, amp, n);
            let y: Vec<f32> = x.iter().map(|&v| o.tick(v, v, 1.0, 0.5, 1.0, 1.0, [0.0; 3], [1.0; 3], SR).0).collect();
            let (a, b) = (rms(&x[24000..]), rms(&y[24000..]));
            if expect_up {
                assert!(b > a * 3.0, "quiet should come up: {a} -> {b}");
            } else {
                assert!(b < a, "loud should come down: {a} -> {b}");
            }
        }
    }

    #[test]
    fn ott_depth_zero_is_transparent_level() {
        let mut o = Ott::new(SR);
        let x = tone(1000.0, 0.3, 48000);
        let y: Vec<f32> = x.iter().map(|&v| o.tick(v, v, 0.0, 0.5, 1.0, 1.0, [0.0; 3], [1.0; 3], SR).0).collect();
        let r = rms(&y[24000..]) / rms(&x[24000..]);
        assert!((r - 1.0).abs() < 0.05, "level {r}");
    }

    #[test]
    fn wide_keeps_lows_mono_and_spreads_highs() {
        let mut w = Wide::new(SR);
        w.set(150.0, SR);
        for (f, want_wide) in [(50.0f32, false), (3000.0, true)] {
            let x = tone(f, 0.5, 48000);
            let mut side = Vec::new();
            for &v in &x {
                let (l, r) = w.tick(v, v, 1.0, 1.0);
                side.push(0.5 * (l - r));
            }
            let s = rms(&side[24000..]);
            if want_wide {
                assert!(s > 0.05, "{f} Hz should get stereo: {s}");
            } else {
                assert!(s < 0.02, "{f} Hz must stay mono: {s}");
            }
        }
    }

    #[test]
    fn vowel_peaks_near_its_first_formant() {
        let gain_at = |pos: f32, f: f32| {
            let mut v = Vowel::default();
            v.set(pos, 0.5, 0.0, SR);
            let x = tone(f, 0.1, 24000);
            let y: Vec<f32> = x.iter().map(|&s| v.tick(s, 0.5)).collect();
            rms(&y[12000..]) / rms(&x[12000..])
        };
        // "A" is open around 800 Hz, "U" isn't.
        assert!(gain_at(0.0, 800.0) > 2.0 * gain_at(1.0, 800.0));
    }

    #[test]
    fn clip_never_exceeds_ceiling() {
        for i in 0..1000 {
            let x = (i as f32 - 500.0) / 50.0;
            for k in [0.0, 0.3, 0.9] {
                assert!(clip(x, k).abs() <= 1.0 + 1e-6);
            }
        }
    }

    #[test]
    fn chaos_is_repeatable_and_in_range() {
        let run = |seed| {
            let mut c = Chaos::new(seed);
            (0..48000).map(|_| c.tick(4.0, 0.7, SR)).collect::<Vec<f32>>()
        };
        let (a, b) = (run(7), run(7));
        assert_eq!(a, b);
        assert!(a.iter().all(|v| (-1.0..=1.0).contains(v)));
        assert_ne!(a, run(8));
    }
}

#[cfg(test)]
mod phat_plus_tests {
    use super::*;
    fn sine(f: f32, n: usize, sr: f32) -> Vec<f32> {
        (0..n).map(|i| (0.5 * (std::f64::consts::TAU * f as f64 * i as f64 / sr as f64).sin()) as f32).collect()
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
    #[test]
    fn disperser_keeps_level_and_shift_moves_the_pitch() {
        let sr = 96000.0;
        let x = sine(1000.0, 96000, sr);
        // DISPERSER all the way: allpasses only, the level stays
        let mut p = PhatPlus::default();
        let y: Vec<f32> = x.iter().map(|v| p.tick(*v, 0.0, 1.0, 300.0, 0.0, 250.0, sr)).collect();
        let d = 20.0 * (rms(&y[48000..]) / rms(&x[48000..])).log10();
        assert!(d.abs() < 0.5, "disperser level {d:.2} dB");
        // SHIFT 20 Hz above 250 Hz: 1 kHz comes out at 1020 Hz (count zero crossings over 0.5 s)
        let mut p = PhatPlus::default();
        let y: Vec<f32> = x.iter().map(|v| p.tick(*v, 0.0, 0.0, 300.0, 20.0, 250.0, sr)).collect();
        let zc = y[48000..].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f32 / 0.5;
        assert!((zc - 1020.0).abs() < 6.0, "shifted to {zc} Hz");
        // everything up: bounded
        let mut p = PhatPlus::default();
        assert!(x.iter().map(|v| p.tick(*v * 2.0, 1.0, 1.0, 120.0, 40.0, 40.0, sr)).all(|v| v.is_finite() && v.abs() < 4.0));
    }
}
