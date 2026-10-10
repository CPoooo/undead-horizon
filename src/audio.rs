//! Fully procedural sound: every effect is synthesized at runtime with WebAudio
//! (oscillators + filtered noise), so the game ships with zero audio assets.

use web_sys::{
    AudioBuffer, AudioContext, AudioContextState, AudioNode, BiquadFilterType, GainNode,
    OscillatorType,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Shoot,
    Hit,
    Crit,
    ZombieDie,
    BigDie,
    Pickup,
    LevelUp,
    Dash,
    Hurt,
    Explosion,
    Lightning,
    Nova,
    WaveStart,
    WaveClear,
    BossRoar,
    UiMove,
    UiSelect,
    GameOver,
    Spit,
    Ultimate,
    Blade,
    Groan,
    Heal,
    UltReady,
}

const SFX_COUNT: usize = 24;

impl Sfx {
    fn idx(self) -> usize {
        self as usize
    }
    /// Minimum seconds between two plays of the same effect (prevents mush).
    fn min_gap(self) -> f64 {
        match self {
            Sfx::Shoot => 0.045,
            Sfx::Hit => 0.03,
            Sfx::Crit => 0.05,
            Sfx::ZombieDie => 0.035,
            Sfx::Pickup => 0.025,
            Sfx::Blade => 0.06,
            Sfx::Groan => 0.9,
            Sfx::Explosion => 0.05,
            Sfx::Lightning => 0.06,
            Sfx::Spit => 0.12,
            _ => 0.0,
        }
    }
}

pub struct Audio {
    ctx: Option<AudioContext>,
    master: Option<GainNode>,
    sfx_bus: Option<GainNode>,
    music_bus: Option<GainNode>,
    noise: Option<AudioBuffer>,
    last: [f64; SFX_COUNT],
    voices_this_frame: u32,
    pub volume: f32,
    pub music_volume: f32,
    pickup_combo: f32,
    pickup_combo_t: f64,
    // music scheduler
    next_beat: f64,
    beat: u32,
    pub intensity: f32,
    pub music_on: bool,
}

impl Audio {
    pub fn new() -> Self {
        Audio {
            ctx: None,
            master: None,
            sfx_bus: None,
            music_bus: None,
            noise: None,
            last: [-1.0; SFX_COUNT],
            voices_this_frame: 0,
            volume: 0.7,
            music_volume: 0.5,
            pickup_combo: 0.0,
            pickup_combo_t: 0.0,
            next_beat: 0.0,
            beat: 0,
            intensity: 0.0,
            music_on: false,
        }
    }

    /// Must be called from a user gesture (browser autoplay policy).
    pub fn unlock(&mut self) {
        if self.ctx.is_none() {
            let _ = self.init();
        }
        if let Some(ctx) = &self.ctx {
            if ctx.state() != AudioContextState::Running {
                let _ = ctx.resume();
            }
        }
    }

    fn init(&mut self) -> Option<()> {
        let ctx = AudioContext::new().ok()?;
        let comp = ctx.create_dynamics_compressor().ok()?;
        comp.threshold().set_value(-14.0);
        comp.knee().set_value(18.0);
        comp.ratio().set_value(5.0);
        comp.attack().set_value(0.003);
        comp.release().set_value(0.18);
        comp.connect_with_audio_node(&ctx.destination()).ok()?;

        let master = ctx.create_gain().ok()?;
        master.gain().set_value(self.volume);
        master.connect_with_audio_node(&comp).ok()?;

        let sfx = ctx.create_gain().ok()?;
        sfx.gain().set_value(1.0);
        sfx.connect_with_audio_node(&master).ok()?;
        let music = ctx.create_gain().ok()?;
        music.gain().set_value(self.music_volume * 0.55);
        music.connect_with_audio_node(&master).ok()?;

        // 1s of white noise, reused by every noise voice.
        let sr = ctx.sample_rate();
        let len = sr as u32;
        let buf = ctx.create_buffer(1, len, sr).ok()?;
        let mut data = vec![0f32; len as usize];
        let mut s: u32 = 0x1234_5678;
        for d in data.iter_mut() {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            *d = (s as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        buf.copy_to_channel(&data, 0).ok()?;

        self.next_beat = ctx.current_time() + 0.1;
        self.noise = Some(buf);
        self.master = Some(master);
        self.sfx_bus = Some(sfx);
        self.music_bus = Some(music);
        self.ctx = Some(ctx);
        Some(())
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        if let Some(m) = &self.master {
            m.gain().set_value(self.volume);
        }
    }
    pub fn set_music_volume(&mut self, v: f32) {
        self.music_volume = v.clamp(0.0, 1.0);
        if let Some(m) = &self.music_bus {
            m.gain().set_value(self.music_volume * 0.55);
        }
    }

    pub fn begin_frame(&mut self) {
        self.voices_this_frame = 0;
    }

    fn now(&self) -> f64 {
        self.ctx.as_ref().map(|c| c.current_time()).unwrap_or(0.0)
    }

    /// Play at screen center.
    pub fn play(&mut self, s: Sfx) {
        self.play_at(s, 0.0, 1.0);
    }

    /// `pan` in [-1,1], `vol` multiplier (distance attenuation).
    pub fn play_at(&mut self, s: Sfx, pan: f32, vol: f32) {
        if self.ctx.is_none() || vol <= 0.02 {
            return;
        }
        let t = self.now();
        let i = s.idx();
        if t - self.last[i] < s.min_gap() {
            return;
        }
        let important = matches!(
            s,
            Sfx::LevelUp
                | Sfx::WaveStart
                | Sfx::WaveClear
                | Sfx::GameOver
                | Sfx::Hurt
                | Sfx::BossRoar
                | Sfx::Ultimate
                | Sfx::UiSelect
                | Sfx::UiMove
                | Sfx::UltReady
        );
        if !important && self.voices_this_frame > 10 {
            return;
        }
        self.voices_this_frame += 1;
        self.last[i] = t;
        let _ = self.synth(s, t + 0.005, pan.clamp(-0.9, 0.9), vol.min(1.0));
    }

    fn out(&self, pan: f32) -> Option<AudioNode> {
        let ctx = self.ctx.as_ref()?;
        let bus = self.sfx_bus.as_ref()?;
        if pan.abs() < 0.02 {
            return Some(bus.clone().into());
        }
        let p = ctx.create_stereo_panner().ok()?;
        p.pan().set_value(pan);
        p.connect_with_audio_node(bus).ok()?;
        Some(p.into())
    }

    #[allow(clippy::too_many_arguments)]
    fn tone(
        &self,
        dest: &AudioNode,
        ty: OscillatorType,
        t: f64,
        f0: f32,
        f1: f32,
        dur: f64,
        gain: f32,
        attack: f64,
    ) -> Option<()> {
        let ctx = self.ctx.as_ref()?;
        let o = ctx.create_oscillator().ok()?;
        o.set_type(ty);
        o.frequency().set_value_at_time(f0, t).ok()?;
        if (f1 - f0).abs() > 0.1 {
            o.frequency()
                .exponential_ramp_to_value_at_time(f1.max(1.0), t + dur)
                .ok()?;
        }
        let g = ctx.create_gain().ok()?;
        g.gain().set_value_at_time(0.0001, t).ok()?;
        g.gain()
            .exponential_ramp_to_value_at_time(gain.max(0.0002), t + attack)
            .ok()?;
        g.gain()
            .exponential_ramp_to_value_at_time(0.0001, t + dur)
            .ok()?;
        o.connect_with_audio_node(&g).ok()?;
        g.connect_with_audio_node(dest).ok()?;
        o.start_with_when(t).ok()?;
        o.stop_with_when(t + dur + 0.02).ok()?;
        Some(())
    }

    #[allow(clippy::too_many_arguments)]
    fn noise(
        &self,
        dest: &AudioNode,
        t: f64,
        ft: BiquadFilterType,
        f0: f32,
        f1: f32,
        q: f32,
        dur: f64,
        gain: f32,
    ) -> Option<()> {
        let ctx = self.ctx.as_ref()?;
        let src = ctx.create_buffer_source().ok()?;
        src.set_buffer(self.noise.as_ref());
        let filt = ctx.create_biquad_filter().ok()?;
        filt.set_type(ft);
        filt.frequency().set_value_at_time(f0, t).ok()?;
        if (f1 - f0).abs() > 0.1 {
            filt.frequency()
                .exponential_ramp_to_value_at_time(f1.max(10.0), t + dur)
                .ok()?;
        }
        filt.q().set_value(q);
        let g = ctx.create_gain().ok()?;
        g.gain().set_value_at_time(gain, t).ok()?;
        g.gain()
            .exponential_ramp_to_value_at_time(0.0001, t + dur)
            .ok()?;
        src.connect_with_audio_node(&filt).ok()?;
        filt.connect_with_audio_node(&g).ok()?;
        g.connect_with_audio_node(dest).ok()?;
        // random offset into the noise buffer so repeated hits differ
        let off = (t * 7.31).fract() * 0.5;
        src.start_with_when_and_grain_offset(t, off).ok()?;
        #[allow(deprecated)]
        src.stop_with_when(t + dur + 0.02).ok()?;
        Some(())
    }

    fn synth(&mut self, s: Sfx, t: f64, pan: f32, v: f32) -> Option<()> {
        use BiquadFilterType as F;
        use OscillatorType as O;
        let d = self.out(pan)?;
        let jit = 1.0 + ((t * 113.7).fract() as f32 - 0.5) * 0.12; // pitch variety
        match s {
            Sfx::Shoot => {
                self.tone(&d, O::Square, t, 880.0 * jit, 180.0, 0.07, 0.07 * v, 0.002)?;
                self.noise(&d, t, F::Highpass, 3000.0, 1500.0, 0.7, 0.05, 0.12 * v)?;
                self.tone(&d, O::Sine, t, 160.0, 50.0, 0.06, 0.18 * v, 0.002)?;
            }
            Sfx::Hit => {
                self.noise(&d, t, F::Bandpass, 1800.0 * jit, 500.0, 2.0, 0.06, 0.28 * v)?;
                self.tone(&d, O::Triangle, t, 240.0 * jit, 90.0, 0.06, 0.14 * v, 0.002)?;
            }
            Sfx::Crit => {
                self.tone(&d, O::Square, t, 1400.0, 2200.0, 0.06, 0.06 * v, 0.002)?;
                self.noise(&d, t, F::Bandpass, 2600.0, 900.0, 3.0, 0.09, 0.3 * v)?;
                self.tone(&d, O::Triangle, t, 300.0, 80.0, 0.09, 0.2 * v, 0.002)?;
            }
            Sfx::ZombieDie => {
                // wet squelch: lowpassed noise sweep + falling growl
                self.noise(&d, t, F::Lowpass, 2200.0 * jit, 200.0, 4.0, 0.22, 0.35 * v)?;
                self.tone(&d, O::Sawtooth, t, 150.0 * jit, 45.0, 0.22, 0.09 * v, 0.004)?;
            }
            Sfx::BigDie => {
                self.noise(&d, t, F::Lowpass, 1600.0, 80.0, 3.0, 0.55, 0.5 * v)?;
                self.tone(&d, O::Sawtooth, t, 110.0, 30.0, 0.6, 0.16 * v, 0.01)?;
                self.tone(&d, O::Sine, t, 90.0, 35.0, 0.5, 0.35 * v, 0.005)?;
            }
            Sfx::Pickup => {
                if t - self.pickup_combo_t > 0.35 {
                    self.pickup_combo = 0.0;
                }
                self.pickup_combo_t = t;
                let semis = (self.pickup_combo as i32 % 15) as f32;
                self.pickup_combo += 1.0;
                let f = 880.0 * 2f32.powf(semis / 12.0 * 0.66);
                self.tone(&d, O::Sine, t, f, f * 1.5, 0.08, 0.12 * v, 0.003)?;
                self.tone(
                    &d,
                    O::Triangle,
                    t + 0.02,
                    f * 2.0,
                    f * 2.0,
                    0.05,
                    0.04 * v,
                    0.003,
                )?;
            }
            Sfx::LevelUp => {
                let notes = [523.25, 659.25, 783.99, 1046.5, 1318.5];
                for (k, n) in notes.iter().enumerate() {
                    let tt = t + k as f64 * 0.065;
                    self.tone(&d, O::Square, tt, *n, *n, 0.22, 0.07, 0.004)?;
                    self.tone(&d, O::Sine, tt, *n * 2.0, *n * 2.0, 0.3, 0.06, 0.004)?;
                }
                self.noise(&d, t, F::Highpass, 6000.0, 9000.0, 0.5, 0.5, 0.06)?;
            }
            Sfx::Dash => {
                self.noise(&d, t, F::Bandpass, 600.0, 3500.0, 1.2, 0.18, 0.35 * v)?;
                self.tone(&d, O::Sine, t, 220.0, 520.0, 0.12, 0.08 * v, 0.004)?;
            }
            Sfx::Hurt => {
                self.tone(&d, O::Sawtooth, t, 220.0, 70.0, 0.25, 0.22, 0.003)?;
                self.noise(&d, t, F::Lowpass, 900.0, 120.0, 2.0, 0.22, 0.5)?;
                self.tone(&d, O::Sine, t, 70.0, 40.0, 0.3, 0.5, 0.004)?;
            }
            Sfx::Explosion => {
                self.noise(&d, t, F::Lowpass, 3000.0 * jit, 60.0, 1.0, 0.7, 0.75 * v)?;
                self.tone(&d, O::Sine, t, 120.0, 28.0, 0.6, 0.6 * v, 0.004)?;
                self.noise(&d, t, F::Highpass, 2000.0, 4000.0, 0.7, 0.12, 0.2 * v)?;
            }
            Sfx::Lightning => {
                self.noise(&d, t, F::Highpass, 5000.0, 1200.0, 1.0, 0.18, 0.4 * v)?;
                self.tone(
                    &d,
                    O::Sawtooth,
                    t,
                    1800.0 * jit,
                    200.0,
                    0.14,
                    0.08 * v,
                    0.002,
                )?;
                self.tone(&d, O::Square, t + 0.03, 900.0, 120.0, 0.1, 0.05 * v, 0.002)?;
            }
            Sfx::Nova => {
                self.tone(&d, O::Sine, t, 1200.0, 300.0, 0.45, 0.14 * v, 0.004)?;
                self.tone(&d, O::Triangle, t, 2400.0, 600.0, 0.35, 0.06 * v, 0.004)?;
                self.noise(&d, t, F::Bandpass, 4000.0, 800.0, 1.5, 0.4, 0.25 * v)?;
            }
            Sfx::WaveStart => {
                // ominous horn swell
                for (k, f) in [110.0f32, 164.81, 220.0].iter().enumerate() {
                    self.tone(
                        &d,
                        O::Sawtooth,
                        t + k as f64 * 0.02,
                        *f,
                        *f * 0.98,
                        1.3,
                        0.07,
                        0.25,
                    )?;
                }
                self.tone(&d, O::Sine, t, 55.0, 50.0, 1.4, 0.3, 0.2)?;
                self.noise(&d, t, F::Lowpass, 400.0, 100.0, 1.0, 1.2, 0.12)?;
            }
            Sfx::WaveClear => {
                let notes = [392.0, 523.25, 659.25, 783.99, 1046.5];
                for (k, n) in notes.iter().enumerate() {
                    let tt = t + k as f64 * 0.09;
                    self.tone(&d, O::Triangle, tt, *n, *n, 0.5, 0.12, 0.005)?;
                    self.tone(&d, O::Sine, tt, *n * 0.5, *n * 0.5, 0.6, 0.08, 0.005)?;
                }
            }
            Sfx::BossRoar => {
                self.tone(&d, O::Sawtooth, t, 90.0, 45.0, 1.6, 0.25, 0.15)?;
                self.tone(&d, O::Sawtooth, t, 93.0, 44.0, 1.6, 0.2, 0.15)?;
                self.noise(&d, t, F::Lowpass, 900.0, 120.0, 6.0, 1.5, 0.5)?;
                self.tone(&d, O::Sine, t, 45.0, 30.0, 1.6, 0.5, 0.1)?;
            }
            Sfx::UiMove => {
                self.tone(&d, O::Square, t, 660.0, 660.0, 0.05, 0.05, 0.002)?;
            }
            Sfx::UiSelect => {
                self.tone(&d, O::Square, t, 660.0, 660.0, 0.06, 0.07, 0.002)?;
                self.tone(&d, O::Square, t + 0.06, 990.0, 990.0, 0.12, 0.07, 0.002)?;
                self.noise(&d, t, F::Highpass, 5000.0, 8000.0, 0.5, 0.2, 0.05)?;
            }
            Sfx::GameOver => {
                let notes = [392.0, 349.23, 311.13, 261.63, 196.0];
                for (k, n) in notes.iter().enumerate() {
                    let tt = t + k as f64 * 0.22;
                    self.tone(&d, O::Sawtooth, tt, *n, *n * 0.97, 0.5, 0.08, 0.01)?;
                    self.tone(&d, O::Sine, tt, *n * 0.5, *n * 0.5, 0.6, 0.15, 0.01)?;
                }
                self.noise(&d, t, F::Lowpass, 600.0, 60.0, 1.0, 1.8, 0.2)?;
            }
            Sfx::Spit => {
                self.noise(&d, t, F::Bandpass, 900.0, 2400.0, 5.0, 0.15, 0.3 * v)?;
                self.tone(&d, O::Sine, t, 300.0, 700.0, 0.1, 0.06 * v, 0.003)?;
            }
            Sfx::Ultimate => {
                self.tone(&d, O::Sawtooth, t, 60.0, 30.0, 1.4, 0.3, 0.01)?;
                self.tone(&d, O::Sine, t, 2000.0, 80.0, 0.9, 0.2, 0.004)?;
                self.noise(&d, t, F::Lowpass, 6000.0, 40.0, 0.8, 1.6, 0.9)?;
                self.noise(&d, t, F::Highpass, 3000.0, 9000.0, 0.5, 0.6, 0.25)?;
            }
            Sfx::Blade => {
                self.noise(&d, t, F::Bandpass, 4200.0 * jit, 6000.0, 6.0, 0.06, 0.2 * v)?;
                self.tone(
                    &d,
                    O::Triangle,
                    t,
                    1600.0 * jit,
                    1200.0,
                    0.05,
                    0.05 * v,
                    0.002,
                )?;
            }
            Sfx::Groan => {
                let f = 85.0 * jit;
                self.tone(&d, O::Sawtooth, t, f, f * 0.8, 0.9, 0.05 * v, 0.2)?;
                self.noise(&d, t, F::Bandpass, 500.0, 300.0, 8.0, 0.9, 0.08 * v)?;
            }
            Sfx::Heal => {
                self.tone(&d, O::Sine, t, 523.25, 1046.5, 0.3, 0.1, 0.02)?;
                self.tone(&d, O::Sine, t + 0.08, 783.99, 1568.0, 0.3, 0.08, 0.02)?;
            }
            Sfx::UltReady => {
                self.tone(&d, O::Triangle, t, 880.0, 880.0, 0.12, 0.09, 0.003)?;
                self.tone(&d, O::Triangle, t + 0.1, 1318.5, 1318.5, 0.25, 0.09, 0.003)?;
            }
        }
        Some(())
    }

    /// Lookahead-scheduled procedural soundtrack. `intensity` 0..1 drives tempo/layers.
    pub fn update_music(&mut self) -> Option<()> {
        if !self.music_on {
            return Some(());
        }
        let now = self.now();
        let ctx = self.ctx.as_ref()?;
        if ctx.state() != AudioContextState::Running {
            return Some(());
        }
        let bus: AudioNode = self.music_bus.as_ref()?.clone().into();
        if self.next_beat < now {
            self.next_beat = now + 0.05;
        }
        let bpm = 92.0 + self.intensity as f64 * 40.0;
        let step = 60.0 / bpm / 2.0; // eighth notes
                                     // A minor-ish dark progression (bass roots)
        let roots = [55.0f32, 55.0, 43.65, 49.0];
        while self.next_beat < now + 0.2 {
            let t = self.next_beat;
            let b = self.beat;
            let bar = (b / 16) as usize % roots.len();
            let root = roots[bar];
            let st = b % 16;
            use BiquadFilterType as F;
            use OscillatorType as O;
            // bass pulse
            if st % 4 == 0 || (self.intensity > 0.45 && st % 4 == 3) {
                self.tone(&bus, O::Sawtooth, t, root, root, step * 1.6, 0.11, 0.01)?;
                self.tone(
                    &bus,
                    O::Sine,
                    t,
                    root * 0.5,
                    root * 0.5,
                    step * 1.8,
                    0.2,
                    0.01,
                )?;
            }
            // kick-like heartbeat
            if st == 0 || st == 3 || (self.intensity > 0.3 && (st == 8 || st == 11)) {
                self.tone(&bus, O::Sine, t, 120.0, 40.0, 0.18, 0.45, 0.003)?;
            }
            // hats when things heat up
            if self.intensity > 0.2 && st % 2 == 1 {
                self.noise(
                    &bus,
                    t,
                    F::Highpass,
                    7000.0,
                    7000.0,
                    0.7,
                    0.04,
                    0.05 + 0.05 * self.intensity,
                )?;
            }
            // snare on 2 & 4 at high intensity
            if self.intensity > 0.55 && (st == 4 || st == 12) {
                self.noise(&bus, t, F::Bandpass, 1800.0, 900.0, 0.8, 0.14, 0.18)?;
            }
            // eerie lead arpeggio
            if st % 8 == 6 && (b / 8) % 2 == 1 {
                let f = root * 8.0 * [1.0, 1.189, 1.498, 1.335][(b as usize / 16) % 4];
                self.tone(&bus, O::Triangle, t, f, f * 0.995, step * 3.0, 0.035, 0.05)?;
            }
            self.next_beat += step;
            self.beat = self.beat.wrapping_add(1);
        }
        Some(())
    }
}
