use chop_shop::dsp::*;
#[test]
fn cpu_all_modules() {
    let sr = 48000.0;
    let (mut ott, mut wide) = (Ott::new(sr), Wide::new(sr));
    wide.set(120.0, sr);
    let (mut ph, mut bz, mut vw, mut ch) = ([Phat::default(); 2], [Buzz::default(); 2], [Vowel::default(); 2], Chaos::new(3));
    let n = (sr * 10.0) as usize;
    let t = std::time::Instant::now();
    let mut acc = 0.0f32;
    for i in 0..n {
        let c = ch.tick(2.0, 0.7, sr);
        if i % 32 == 0 {
            for k in 0..2 { bz[k].set(1500.0 * (1.0 + c * 0.5), sr); vw[k].set(0.5 + c * 0.25, 0.5, 0.0, sr); }
        }
        let x = ((i * 7919) % 1000) as f32 / 1000.0 - 0.5;
        let (a, b) = (ph[0].tick(x, 12.0, 0.2, sr), ph[1].tick(-x, 12.0, 0.2, sr));
        let (a, b) = ott.tick(a, b, 0.5, 0.5, 1.0, 1.0, [0.0; 3], [1.0; 3], sr);
        let (a, b) = (bz[0].tick(a, 12.0, 0.5), bz[1].tick(b, 12.0, 0.5));
        let (a, b) = (vw[0].tick(a, 0.5), vw[1].tick(b, 0.5));
        let (a, b) = wide.tick(a, b, 1.5, 0.5);
        acc += clip(a, 0.3) + clip(b, 0.3);
    }
    println!("CHOPSHOP all modules: {:.2}% of one core ({acc:.1})", t.elapsed().as_secs_f64() / 10.0 * 100.0);
}
