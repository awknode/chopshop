# CHOPSHOP manual

## Signal flow

```
INPUT ─► [LOW KEEP split] ─► PHAT ─► OTT ─► BUZZ ─► VOWEL ─► WIDE ─► CLIP ─► (sub back in) ─► MIX / OUTPUT ─► SAFE
                         CHAOS (random LFO) ─► PHAT drive, BUZZ freq, VOWEL
```

Each module has a power LED on its strip.

## Modules

| Module | Controls |
|---|---|
| **PHAT** | DRIVE, COLOR, MIX, DRY, WIDE, BUZZ, DISP (disperser) + DISP HZ, SHIFT (frequency shifter), X-FREQ (crossover above which the disperser and shifter act). Mode chips STRONK, RIDDIM, OFF and INIT set the knobs. |
| **OTT** | DEPTH, TIME, UP, DOWN, band crossovers, and a per-band amount (by default the mids get it all, the lows none and the highs a touch). Drag a band on the screen up or down to set its output gain. |
| **BUZZ** | FREQ, DRIVE, MIX. |
| **VOWEL** | VOWEL (A, E, I, O, U), RES, SHIFT, MIX. |
| **CHAOS** | RATE, SMOOTH, SEED, plus how far it pushes VOWEL, DRIVE and BUZZ. |
| **WIDE** | WIDTH, SPREAD, MONO BELOW. |
| **CLIP** | DRIVE, KNEE. |

## Global

- **INPUT / OUTPUT** with meters.
- **LOW KEEP:** below this frequency the signal skips the rack.
- **MATCH LEVEL:** output follows input loudness.
- **OVERSAMPLING:** OFF / 2x / 4x / 8x for the whole chain, dry/wet mix included.
- **MIX**, power (bypass) and SAFE.
