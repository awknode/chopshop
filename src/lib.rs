//! CHOPSHOP: a bass-fattening rack in one plugin (awk studios).
//!
//! Chain:  INPUT > PHAT > OTT > BUZZ > VOWEL > WIDE > CLIP > MIX/OUTPUT
//! CHAOS is a random LFO that can push PHAT drive, BUZZ freq and VOWEL.
//! LOW KEEP (an addition): everything below its frequency skips the rack
//! untouched, so the sub stays clean and centred while the mids get chewed.
//! OVERSAMPLING (OFF/2x/4x/8x) runs the whole chain, dry/wet MIX included, at the
//! higher rate so PHAT, BUZZ and CLIP don't alias. Header power = host bypass.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

mod analyzer;
mod awk_ui;
pub mod dsp;
mod editor;
mod oversample;
mod presets;
#[cfg(test)]
mod style;

use awk_ui::{BypassRamp, Meters, PeakFollower, SpecRing};
use dsp::*;
use oversample::{Oversampler, MAX_FACTOR};

pub struct Shared {
    pub chaos: AtomicU32,
    pub ott_gr: [AtomicU32; 3],
    pub meters: Meters,
    pub spec: SpecRing,
}

pub const PHAT_MODES: [&str; 4] = ["STRONK", "RIDDIM", "OFF", "INIT"];
/// SHIFT knob -> Hz (gentle at the bottom: a few Hz is already wide)
pub fn shift_hz(v: f32) -> f32 {
    40.0 * v * v
}

fn db_param(name: &str, def: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Linear { min, max })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_unit(" dB")
        .with_value_to_string(Arc::new(|v| format!("{v:+.1}")))
}
fn pct(name: &str, def: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}
fn hz(name: &str, def: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-2.0) })
        .with_smoother(SmoothingStyle::Logarithmic(30.0))
        .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
        .with_string_to_value(formatters::s2v_f32_hz_then_khz())
}

#[derive(Params)]
pub struct ChopParams {
    #[persist = "editor-state-v2"]
    pub editor_state: Arc<EguiState>,

    #[id = "in"]
    pub input: FloatParam,
    #[id = "lowkeep"]
    pub low_keep: FloatParam,

    #[id = "phat"]
    pub phat_on: BoolParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "color"]
    pub color: FloatParam,
    #[id = "phmix"]
    pub phat_mix: FloatParam,
    // phat-rack spec: PHAT (= drive), DRY, WIDE, BUZZ / DISPERSER, DISPERSER FREQ, SHIFT, SHIFT X-FREQ
    #[id = "phdry"]
    pub ph_dry: FloatParam,
    #[id = "phwide"]
    pub ph_wide: FloatParam,
    #[id = "phbuzz"]
    pub ph_buzz: FloatParam,
    #[id = "phdisp"]
    pub ph_disp: FloatParam,
    #[id = "phdf"]
    pub ph_dfreq: FloatParam,
    #[id = "phshift"]
    pub ph_shift: FloatParam,
    #[id = "phxf"]
    pub ph_xf: FloatParam,
    /// STRONK / RIDDIM / OFF / INIT (which set the knobs; remembered for the display)
    #[id = "phmode"]
    pub ph_mode: IntParam,

    #[id = "ott"]
    pub ott_on: BoolParam,
    #[id = "depth"]
    pub depth: FloatParam,
    #[id = "time"]
    pub time: FloatParam,
    #[id = "up"]
    pub up: FloatParam,
    #[id = "down"]
    pub down: FloatParam,
    #[id = "olow"]
    pub ott_low: FloatParam,
    #[id = "omid"]
    pub ott_mid: FloatParam,
    #[id = "ohigh"]
    pub ott_high: FloatParam,
    /// How much OTT each band gets. Default: mids get it all, lows none, highs a touch.
    #[id = "oal"]
    pub ott_amt_low: FloatParam,
    #[id = "oam"]
    pub ott_amt_mid: FloatParam,
    #[id = "oah"]
    pub ott_amt_high: FloatParam,

    #[id = "buzz"]
    pub buzz_on: BoolParam,
    #[id = "bfreq"]
    pub buzz_freq: FloatParam,
    #[id = "bdrive"]
    pub buzz_drive: FloatParam,
    #[id = "bmix"]
    pub buzz_mix: FloatParam,

    #[id = "vowel"]
    pub vowel_on: BoolParam,
    #[id = "vpos"]
    pub vowel: FloatParam,
    #[id = "vres"]
    pub vowel_res: FloatParam,
    #[id = "vshift"]
    pub vowel_shift: FloatParam,
    #[id = "vmix"]
    pub vowel_mix: FloatParam,

    #[id = "chaos"]
    pub chaos_on: BoolParam,
    #[id = "crate"]
    pub chaos_rate: FloatParam,
    #[id = "csmooth"]
    pub chaos_smooth: FloatParam,
    #[id = "cvowel"]
    pub chaos_vowel: FloatParam,
    #[id = "cdrive"]
    pub chaos_drive: FloatParam,
    #[id = "cbuzz"]
    pub chaos_buzz: FloatParam,
    #[id = "cseed"]
    pub chaos_seed: IntParam,

    #[id = "wide"]
    pub wide_on: BoolParam,
    #[id = "width"]
    pub width: FloatParam,
    #[id = "spread"]
    pub spread: FloatParam,
    #[id = "mono"]
    pub mono_below: FloatParam,

    #[id = "clip"]
    pub clip_on: BoolParam,
    #[id = "cdb"]
    pub clip_drive: FloatParam,
    #[id = "knee"]
    pub knee: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "out"]
    pub output: FloatParam,
    #[id = "match"]
    pub match_level: BoolParam,

    #[id = "os"]
    pub os: IntParam,
    #[id = "bypass"]
    pub bypass: BoolParam,
    /// SAFE: output limiter at -0.1 dBFS (footer switch, on by default).
    #[id = "safe"]
    pub safe: BoolParam,
}

impl Default for ChopParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(editor::WIDTH, editor::HEIGHT),
            input: db_param("Input", 0.0, -24.0, 24.0),
            low_keep: FloatParam::new("Low Keep", 20.0, FloatRange::Skewed { min: 20.0, max: 300.0, factor: FloatRange::skew_factor(-1.0) })
                .with_smoother(SmoothingStyle::Logarithmic(30.0))
                .with_value_to_string(Arc::new(|v| if v <= 20.5 { "OFF".into() } else { format!("{v:.0} Hz") })),
            phat_on: BoolParam::new("Phat On", true),
            drive: db_param("Phat Drive", 6.0, 0.0, 36.0),
            color: FloatParam::new("Phat Color", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(Arc::new(|v| if v.abs() < 0.02 { "FLAT".into() } else if v < 0.0 { format!("DARK {:.0}", -v * 100.0) } else { format!("BRIGHT {:.0}", v * 100.0) })),
            phat_mix: pct("Phat Mix", 1.0),
            ph_dry: pct("Phat Dry", 0.0),
            ph_wide: pct("Phat Wide", 0.0),
            ph_buzz: pct("Phat Buzz", 0.0),
            ph_disp: pct("Disperser", 0.0),
            ph_dfreq: hz("Disperser Freq", 250.0, 40.0, 4000.0),
            ph_shift: FloatParam::new("Shift", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(Arc::new(|v| format!("{:.1} Hz", shift_hz(v))))
                .with_unit(""),
            ph_xf: hz("Shift X-Freq", 250.0, 40.0, 2000.0),
            ph_mode: IntParam::new("Phat Mode", 3, IntRange::Linear { min: 0, max: 3 }).with_value_to_string(Arc::new(|v| PHAT_MODES[v.clamp(0, 3) as usize].to_string())).non_automatable(),
            ott_on: BoolParam::new("OTT On", true),
            depth: pct("OTT Depth", 0.35),
            time: pct("OTT Time", 0.5),
            up: pct("OTT Upward", 1.0),
            down: pct("OTT Downward", 1.0),
            ott_low: db_param("OTT Low", 0.0, -12.0, 12.0),
            ott_mid: db_param("OTT Mid", 0.0, -12.0, 12.0),
            ott_high: db_param("OTT High", 0.0, -12.0, 12.0),
            ott_amt_low: pct("OTT Low Amount", 0.0),
            ott_amt_mid: pct("OTT Mid Amount", 1.0),
            ott_amt_high: pct("OTT High Amount", 0.25),
            buzz_on: BoolParam::new("Buzz On", false),
            buzz_freq: hz("Buzz Freq", 1500.0, 200.0, 8000.0),
            buzz_drive: db_param("Buzz Drive", 12.0, 0.0, 36.0),
            buzz_mix: pct("Buzz Mix", 0.5),
            vowel_on: BoolParam::new("Vowel On", false),
            vowel: FloatParam::new("Vowel", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(30.0))
                .with_value_to_string(Arc::new(|v| {
                    let p = v * 4.0;
                    let i = (p.round() as usize).min(4);
                    if (p - p.round()).abs() < 0.12 { VOWELS[i].to_string() } else { format!("{} > {}", VOWELS[(p as usize).min(3)], VOWELS[(p as usize + 1).min(4)]) }
                })),
            vowel_res: pct("Vowel Res", 0.5),
            vowel_shift: FloatParam::new("Vowel Shift", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(30.0))
                .with_value_to_string(Arc::new(|v| format!("{:+.0} st", v * 12.0))),
            vowel_mix: pct("Vowel Mix", 1.0),
            chaos_on: BoolParam::new("Chaos On", false),
            chaos_rate: FloatParam::new("Chaos Rate", 2.0, FloatRange::Skewed { min: 0.05, max: 30.0, factor: FloatRange::skew_factor(-2.0) })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            chaos_smooth: FloatParam::new("Chaos Smooth", 0.7, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(Arc::new(|v| if v < 0.05 { "STEPS".into() } else if v > 0.95 { "GLIDE".into() } else { format!("{:.0}% GLIDE", v * 100.0) })),
            chaos_vowel: pct("Chaos > Vowel", 0.5),
            chaos_drive: pct("Chaos > Drive", 0.0),
            chaos_buzz: pct("Chaos > Buzz", 0.0),
            chaos_seed: IntParam::new("Chaos Seed", 1, IntRange::Linear { min: 1, max: 999 }),
            wide_on: BoolParam::new("Wide On", true),
            width: FloatParam::new("Width", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit("%")
                .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0))),
            spread: pct("Spread", 0.0),
            mono_below: hz("Mono Below", 120.0, 20.0, 500.0),
            clip_on: BoolParam::new("Clip On", true),
            clip_drive: db_param("Clip Drive", 0.0, 0.0, 18.0),
            knee: pct("Clip Knee", 0.3),
            mix: pct("Mix", 1.0),
            output: db_param("Output", 0.0, -24.0, 12.0),
            match_level: BoolParam::new("Match Level", false),
            // 2x by default: PHAT / BUZZ / CLIP alias into fizz without it
            os: IntParam::new("Oversampling", 1, IntRange::Linear { min: 0, max: 3 })
                .with_value_to_string(Arc::new(|v| awk_ui::OS_NAMES[v.clamp(0, 3) as usize].to_string()))
                .with_string_to_value(Arc::new(|s| awk_ui::OS_NAMES.iter().position(|n| n.eq_ignore_ascii_case(s.trim())).map(|i| i as i32))),
            bypass: awk_ui::bypass_param(),
            safe: awk_ui::safe_param(),
        }
    }
}

pub struct ChopShop {
    params: Arc<ChopParams>,
    shared: Arc<Shared>,
    sr: f32,
    keep: [Lr4; 2],
    keep_f: f32,
    phat: [Phat; 2],
    phx: [PhatPlus; 2],
    ott: Option<Ott>,
    buzz: [Buzz; 2],
    vowel: [Vowel; 2],
    chaos: Chaos,
    seed: i32,
    wide: Option<Wide>,
    mono_f: f32,
    ctr: u32,
    env_in: f32,
    env_out: f32,
    auto: f32,
    /// Rate the rack runs at (sr x oversampling factor).
    rate: f32,
    osf: usize,
    ovs: [Oversampler; 2],
    byp: BypassRamp,
    meter: PeakFollower,
    ceil: awk_ui::Ceiling,
    ch_s: f32,
    ch_k: f32,
    last_bf: f32,
    last_v: (f32, f32, f32),
    lvl_c: f32,
}

/// Control values for one base-rate sample, shared by its oversampled sub-samples.
struct Vals {
    in_g: f32,
    drive: f32,
    color: f32,
    pm: f32,
    pdry: f32,
    pwide: f32,
    pbuzz: f32,
    pdisp: f32,
    pdf: f32,
    pshift: f32,
    pxf: f32,
    depth: f32,
    time: f32,
    up: f32,
    down: f32,
    bands: [f32; 3],
    amt: [f32; 3],
    bd: f32,
    bm: f32,
    vm: f32,
    res: f32,
    w: f32,
    s: f32,
    cd: f32,
    knee: f32,
    mix: f32,
    og: f32,
    on: f32,
    lvl_c: f32,
}
#[derive(Clone, Copy)]
struct On {
    phat: bool,
    ott: bool,
    buzz: bool,
    vowel: bool,
    wide: bool,
    clip: bool,
    matching: bool,
}

impl Default for ChopShop {
    fn default() -> Self {
        Self {
            params: Arc::new(ChopParams::default()),
            shared: Arc::new(Shared {
                chaos: AtomicU32::new(0),
                ott_gr: [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
                meters: Meters::default(),
                spec: SpecRing::new(analyzer::FFT),
            }),
            sr: 48000.0,
            keep: [Lr4::default(); 2],
            keep_f: -1.0,
            phat: [Phat::default(); 2],
            phx: [PhatPlus::default(); 2],
            ott: None,
            buzz: [Buzz::default(); 2],
            vowel: [Vowel::default(); 2],
            chaos: Chaos::new(1),
            seed: 1,
            wide: None,
            mono_f: -1.0,
            ctr: 0,
            env_in: 0.0,
            env_out: 0.0,
            auto: 1.0,
            rate: 48000.0,
            osf: 1,
            ovs: [Oversampler::new(1), Oversampler::new(1)],
            byp: BypassRamp::default(),
            meter: PeakFollower::default(),
            ceil: awk_ui::Ceiling::default(),
            ch_s: 0.0,
            ch_k: 0.0,
            last_bf: -1.0,
            last_v: (-1.0, -1.0, -1.0),
            lvl_c: 0.0,
        }
    }
}

impl Plugin for ChopShop {
    const NAME: &'static str = "ChopShop";
    const VENDOR: &'static str = "awk studios";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout { main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() },
        AudioIOLayout { main_input_channels: NonZeroU32::new(1), main_output_channels: NonZeroU32::new(1), ..AudioIOLayout::const_default() },
    ];
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _a: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.shared.clone())
    }

    fn initialize(&mut self, _l: &AudioIOLayout, cfg: &BufferConfig, ctx: &mut impl InitContext<Self>) -> bool {
        self.sr = cfg.sample_rate;
        self.osf = oversample::factor_for(self.params.os.value());
        self.rebuild();
        self.byp.set_sr(self.sr);
        self.meter.set_sr(self.sr);
        self.ceil.set_sr(self.sr);
        self.shared.spec.sr.store(self.sr.to_bits(), Ordering::Relaxed);
        ctx.set_latency_samples(oversample::latency(self.osf) as u32);
        true
    }

    fn reset(&mut self) {
        self.rebuild();
        self.chaos.reseed(self.seed as u32);
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, ctx: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let p = self.params.clone();
        let f = oversample::factor_for(p.os.value());
        if f != self.osf {
            self.osf = f;
            self.rebuild();
            ctx.set_latency_samples(oversample::latency(f) as u32);
        }
        let seed = p.chaos_seed.value();
        if seed != self.seed {
            self.seed = seed;
            self.chaos.reseed(seed as u32);
        }
        let nch = buffer.channels();
        let mut ch = 0.0f32;
        for mut frame in buffer.iter_samples() {
            let x0 = *frame.get_mut(0).unwrap();
            let x1 = if nch > 1 { *frame.get_mut(1).unwrap() } else { x0 };
            let (oa, ob) = self.frame(x0, x1, &p, &mut ch);
            let ob = if nch > 1 { ob } else { oa };
            self.meter.feed(x0, x1, oa, ob);
            self.shared.spec.push(0.5 * (oa + ob));
            *frame.get_mut(0).unwrap() = oa;
            if nch > 1 {
                *frame.get_mut(1).unwrap() = ob;
            }
        }
        let on_ott = p.ott_on.value();
        let sh = &self.shared;
        sh.chaos.store(ch.to_bits(), Ordering::Relaxed);
        self.meter.store(&sh.meters);
        self.ceil.store(&sh.meters);
        if let Some(o) = self.ott.as_ref() {
            for i in 0..3 {
                sh.ott_gr[i].store(if on_ott { o.gr_db[i] } else { 0.0f32 }.to_bits(), Ordering::Relaxed);
            }
        }
        ProcessStatus::Normal
    }
}

impl ChopShop {
    /// One base-rate stereo sample through CHAOS, the oversampled rack and SAFE.
    pub fn frame(&mut self, x0: f32, x1: f32, p: &ChopParams, ch: &mut f32) -> (f32, f32) {
        let (sr, rate) = (self.sr, self.rate);
        let f = self.osf;
        let on = On {
            phat: p.phat_on.value(),
            ott: p.ott_on.value(),
            buzz: p.buzz_on.value(),
            vowel: p.vowel_on.value(),
            wide: p.wide_on.value(),
            clip: p.clip_on.value(),
            matching: p.match_level.value(),
        };
        let chaos_on = p.chaos_on.value();
        let bypassed = p.bypass.value();
        let lvl_c = self.lvl_c;
        let mut buf = [[0.0f32; MAX_FACTOR]; 2];
        {

            // CHAOS (base rate) + slow coefficient updates (every 32 samples).
            // CHAOS, slewed (~3 ms) so even STEPS never jump in a single sample
            let raw = if chaos_on { self.chaos.tick(p.chaos_rate.value(), p.chaos_smooth.smoothed.next(), sr) } else { 0.0 };
            self.ch_s = raw + (self.ch_s - raw) * self.ch_k;
            *ch = self.ch_s;
            let bf_s = p.buzz_freq.smoothed.next();
            let vp_s = p.vowel.smoothed.next();
            let vs_s = p.vowel_shift.smoothed.next();
            let vres = p.vowel_res.smoothed.next();
            let (cb, cv, cd) = (p.chaos_buzz.smoothed.next(), p.chaos_vowel.smoothed.next(), p.chaos_drive.smoothed.next());
            if self.ctr % 32 == 0 {
                let kf = p.low_keep.smoothed.next();
                if (kf - self.keep_f).abs() > 0.01 {
                    for k in self.keep.iter_mut() {
                        k.set(kf.max(20.0), rate);
                    }
                    self.keep_f = kf;
                }
                let mf = p.mono_below.value();
                if (mf - self.mono_f).abs() > 0.01 {
                    if let Some(w) = self.wide.as_mut() {
                        w.set(mf, rate);
                    }
                    self.mono_f = mf;
                }
            } else {
                p.low_keep.smoothed.next();
            }
            // BUZZ / VOWEL follow every sample (only when they actually move):
            // updating them in steps is what made CHAOS crackle.
            if on.buzz {
                let bf = (bf_s * 2f32.powf(*ch * cb * 2.0)).clamp(100.0, 12000.0);
                if bf != self.last_bf {
                    for b in self.buzz.iter_mut() {
                        b.set(bf, rate);
                    }
                    self.last_bf = bf;
                }
            }
            if on.vowel {
                let k = ((vp_s + *ch * cv * 0.5).clamp(0.0, 1.0), vres, vs_s);
                if k != self.last_v {
                    for v in self.vowel.iter_mut() {
                        v.set(k.0, k.1, k.2, rate);
                    }
                    self.last_v = k;
                }
            }
            self.ctr = self.ctr.wrapping_add(1);

            let v = Vals {
                in_g: db_to_gain(p.input.smoothed.next()),
                drive: (p.drive.smoothed.next() + *ch * cd * 12.0).max(0.0),
                color: p.color.smoothed.next(),
                pm: p.phat_mix.smoothed.next(),
                pdry: p.ph_dry.smoothed.next(),
                pwide: p.ph_wide.smoothed.next(),
                pbuzz: p.ph_buzz.smoothed.next(),
                pdisp: p.ph_disp.smoothed.next(),
                pdf: p.ph_dfreq.smoothed.next(),
                pshift: shift_hz(p.ph_shift.smoothed.next()),
                pxf: p.ph_xf.smoothed.next(),
                depth: p.depth.smoothed.next(),
                time: p.time.value(),
                up: p.up.smoothed.next(),
                down: p.down.smoothed.next(),
                bands: [p.ott_low.smoothed.next(), p.ott_mid.smoothed.next(), p.ott_high.smoothed.next()],
                amt: [p.ott_amt_low.smoothed.next(), p.ott_amt_mid.smoothed.next(), p.ott_amt_high.smoothed.next()],
                bd: p.buzz_drive.smoothed.next(),
                bm: p.buzz_mix.smoothed.next(),
                vm: p.vowel_mix.smoothed.next(),
                res: vres,
                w: p.width.smoothed.next(),
                s: p.spread.smoothed.next(),
                cd: db_to_gain(p.clip_drive.smoothed.next()),
                knee: p.knee.smoothed.next(),
                mix: p.mix.smoothed.next(),
                og: db_to_gain(p.output.smoothed.next()),
                on: self.byp.next(bypassed),
                lvl_c,
            };

            self.ovs[0].up(x0, &mut buf[0]);
            self.ovs[1].up(x1, &mut buf[1]);
            for k in 0..f {
                let (a, b) = self.rack(buf[0][k], buf[1][k], &v, on);
                buf[0][k] = a;
                buf[1][k] = b;
            }
            let oa = self.ovs[0].down(&mut buf[0]);
            let ob = self.ovs[1].down(&mut buf[1]);
            // SAFE: never above -0.1 dBFS.
            self.ceil.tick(oa, ob, p.safe.value())
        }
    }

    /// (Re)builds every rate-dependent part for the current oversampling factor.
    fn rebuild(&mut self) {
        let rate = self.sr * self.osf as f32;
        self.rate = rate;
        self.ch_k = coef(3.0, self.sr);
        self.lvl_c = coef(300.0, rate);
        self.ott = Some(Ott::new(rate));
        self.wide = Some(Wide::new(rate));
        self.keep = [Lr4::default(); 2];
        self.keep_f = -1.0;
        self.mono_f = -1.0;
        self.phat = [Phat::default(); 2];
        self.phx = [PhatPlus::default(); 2];
        self.buzz = [Buzz::default(); 2];
        self.last_bf = -1.0;
        self.last_v = (-1.0, -1.0, -1.0);
        self.vowel = [Vowel::default(); 2];
        self.ovs = [Oversampler::new(self.osf), Oversampler::new(self.osf)];
        self.ctr = 0;
    }

    /// The rack for one (possibly oversampled) stereo sample. `xl`/`xr` are the
    /// untouched input; the result already has MIX, OUTPUT and bypass applied, so
    /// the dry path goes through the same filters and stays phase-aligned.
    #[inline]
    fn rack(&mut self, xl: f32, xr: f32, v: &Vals, on: On) -> (f32, f32) {
        let rate = self.rate;
        let (l, r) = (xl * v.in_g, xr * v.in_g);
        // LOW KEEP: split off the sub, it skips the rack.
        let keep_on = self.keep_f > 20.5;
        let ((kl, wl), (kr, wr)) = if keep_on { (self.keep[0].split(l), self.keep[1].split(r)) } else { ((0.0, l), (0.0, r)) };
        let (mut a, mut b) = (wl, wr);
        if on.phat {
            let pa = self.phat[0].tick(a, v.drive, v.color, rate);
            let pb = self.phat[1].tick(b, v.drive, v.color, rate);
            let pa = self.phx[0].tick(pa, v.pbuzz, v.pdisp, v.pdf, v.pshift, v.pxf, rate);
            let pb = self.phx[1].tick(pb, v.pbuzz, v.pdisp, v.pdf, -v.pshift, v.pxf, rate);
            // WIDE: open up the side of the phat signal
            let (m, sd) = (0.5 * (pa + pb), 0.5 * (pa - pb) * (1.0 + 2.0 * v.pwide));
            let (pa, pb) = (m + sd, m - sd);
            let (da, db) = (a, b);
            a += (pa - a) * v.pm;
            b += (pb - b) * v.pm;
            // DRY: the clean signal layered back under it
            if v.pdry > 0.001 {
                let n = 1.0 / (1.0 + v.pdry * v.pdry).sqrt();
                a = (a + da * v.pdry) * n;
                b = (b + db * v.pdry) * n;
            }
        }
        if on.ott {
            if let Some(o) = self.ott.as_mut() {
                (a, b) = o.tick(a, b, v.depth, v.time, v.up, v.down, v.bands, v.amt, rate);
            }
        }
        if on.buzz {
            a = self.buzz[0].tick(a, v.bd, v.bm);
            b = self.buzz[1].tick(b, v.bd, v.bm);
        }
        if on.vowel {
            let va = self.vowel[0].tick(a, v.res);
            let vb = self.vowel[1].tick(b, v.res);
            a += (va - a) * v.vm;
            b += (vb - b) * v.vm;
        }
        // Sub back in, then WIDE (its MONO BELOW keeps the low end centred).
        a += kl;
        b += kr;
        if on.wide {
            if let Some(wd) = self.wide.as_mut() {
                (a, b) = wd.tick(a, b, v.w, v.s);
            }
        }
        if on.clip {
            a = clip(a * v.cd, v.knee);
            b = clip(b * v.cd, v.knee);
        }
        // MATCH LEVEL: rack output follows the input loudness.
        let lin = 0.5 * (l * l + r * r);
        let lout = 0.5 * (a * a + b * b);
        self.env_in = lin + (self.env_in - lin) * v.lvl_c;
        self.env_out = lout + (self.env_out - lout) * v.lvl_c;
        let target = if on.matching && self.env_out > 1e-9 && self.env_in > 1e-9 { (self.env_in / self.env_out).sqrt().clamp(0.125, 8.0) } else { 1.0 };
        self.auto = target + (self.auto - target) * v.lvl_c;
        a *= self.auto;
        b *= self.auto;
        // MIX + OUTPUT, then the bypass crossfade back to the untouched input.
        let oa = (l + (a - l) * v.mix) * v.og;
        let ob = (r + (b - r) * v.mix) * v.og;
        (xl + (oa - xl) * v.on, xr + (ob - xr) * v.on)
    }
}

impl ClapPlugin for ChopShop {
    const CLAP_ID: &'static str = "com.awknode.chopshop";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Bass fattening rack");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo, ClapFeature::Distortion];
}

impl Vst3Plugin for ChopShop {
    const VST3_CLASS_ID: [u8; 16] = *b"AwknodeChopShop1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_vst3!(ChopShop);

#[cfg(test)]
mod tests {
    use super::*;

    /// With every module off, the rack is a pure delay of exactly the latency we
    /// report, at every oversampling factor, for both MIX 100% and MIX 0%: so the
    /// dry and wet paths stay in phase and FL's delay compensation lines up.
    #[test]
    fn oversampled_rack_is_aligned_with_reported_latency() {
        for f in [1usize, 2, 4, 8] {
            for mix in [1.0f32, 0.0, 0.5] {
                let mut cs = ChopShop::default();
                cs.sr = 48000.0;
                cs.osf = f;
                cs.rebuild();
                let rate = cs.rate;
                let v = Vals { in_g: 1.0, drive: 0.0, color: 0.0, pm: 0.0, pdry: 0.0, pwide: 0.0, pbuzz: 0.0, pdisp: 0.0, pdf: 250.0, pshift: 0.0, pxf: 250.0, depth: 0.0, time: 0.5, up: 0.0, down: 0.0, bands: [0.0; 3], amt: [1.0; 3], bd: 0.0, bm: 0.0, vm: 0.0, res: 0.5, w: 1.0, s: 0.0, cd: 1.0, knee: 0.3, mix, og: 1.0, on: 1.0, lvl_c: coef(300.0, rate) };
                let off = On { phat: false, ott: false, buzz: false, vowel: false, wide: false, clip: false, matching: false };
                let d = oversample::latency(f);
                let mut buf = [[0.0f32; MAX_FACTOR]; 2];
                let x: Vec<f32> = (0..4800).map(|n| 0.5 * (2.0 * std::f32::consts::PI * 997.0 * n as f32 / 48000.0).sin()).collect();
                let mut y = Vec::new();
                for &s in &x {
                    cs.ovs[0].up(s, &mut buf[0]);
                    cs.ovs[1].up(s, &mut buf[1]);
                    for k in 0..f {
                        let (a, b) = cs.rack(buf[0][k], buf[1][k], &v, off);
                        buf[0][k] = a;
                        buf[1][k] = b;
                    }
                    y.push(cs.ovs[0].down(&mut buf[0]));
                }
                let err = (1000..4800).map(|n| (y[n] - x[n - d]).abs()).fold(0.0f32, f32::max);
                assert!(err < 2e-3, "factor {f} mix {mix}: error {err}");
            }
        }
    }

    /// The whole chain with every module maxed on loud noise: finite and
    /// never over 0 dBFS when CLIP is on.
    #[test]
    fn whole_rack_stays_finite_and_clipped() {
        let sr = 48000.0;
        let mut ott = Ott::new(sr);
        let mut wide = Wide::new(sr);
        wide.set(120.0, sr);
        let mut ph = Phat::default();
        let mut bz = Buzz::default();
        bz.set(1500.0, sr);
        let mut vw = Vowel::default();
        vw.set(0.3, 1.0, 0.0, sr);
        let mut seed = 1u32;
        for _ in 0..48000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = ((seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * 4.0;
            let a = ph.tick(x, 36.0, 1.0, sr);
            let (a, b) = ott.tick(a, a, 1.0, 0.0, 1.0, 1.0, [12.0; 3], [1.0; 3], sr);
            let a = bz.tick(a, 36.0, 1.0);
            let a = vw.tick(a, 1.0);
            let (a, b) = wide.tick(a, b, 2.0, 1.0);
            let (a, b) = (clip(a, 0.3), clip(b, 0.3));
            assert!(a.is_finite() && b.is_finite() && a.abs() <= 1.0 && b.abs() <= 1.0);
        }
    }
}

#[cfg(test)]
mod audit;
#[cfg(test)]
mod shot_test;

#[cfg(test)]
mod dump_defaults;

#[cfg(test)]
mod drag_test;

#[cfg(test)]
pub(crate) fn audit_extra_reset(p: &ChopParams) {
    for f in [&p.ott_amt_low, &p.ott_amt_mid, &p.ott_amt_high] {
        f.smoothed.reset(f.value());
    }
}
