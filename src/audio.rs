//! Fully procedural sound: every effect is synthesized at runtime with WebAudio
//! (oscillators + filtered noise), so the game ships with zero audio assets.

use web_sys::{
    AudioBuffer, AudioContext, AudioContextState, AudioNode, BiquadFilterNode, BiquadFilterType,
    GainNode, OscillatorType,
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
    ShootHeavy,
    /// party lobby: everyone readied up
    ReadyUp,
    /// co-op match launch sting (host pressed start)
    MatchStart,
    /// kill-streak / multi-kill stinger
    Stinger,
}

const SFX_COUNT: usize = 28;

impl Sfx {
    fn idx(self) -> usize {
        self as usize
    }
    /// Minimum seconds between two plays of the same effect (prevents mush).
    fn min_gap(self) -> f64 {
        match self {
            Sfx::Shoot => 0.045,
            Sfx::ShootHeavy => 0.06,
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
    /// music stems: rhythm (always), percussion, lead (fade in with intensity)
    stems: Option<[GainNode; 3]>,
    /// low-health muffle on everything
    muffle: Option<BiquadFilterNode>,
    /// 0 = healthy, 1 = nearly dead (muffle + heartbeat)
    pub low_hp: f32,
    next_heart: f64,
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
    /// which tune plays: gameplay score or the lobby theme
    track: Track,
    /// index into LOBBY_TUNES
    pub lobby_tune: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Drums {
    Shuffle,
    Disco,
    Halftime,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lead {
    Theremin,
    Bell,
    Square,
}

/// A lobby tune: tempo, feel, 4 chords (bass notes) and an 8-bar melody.
pub struct LobbyTune {
    pub name: &'static str,
    pub bpm: f64,
    /// 0 = straight 16ths, 0.24 = heavy swing
    pub swing: f64,
    pub chords: [[i32; 4]; 4],
    pub melody: &'static [(u32, u32, i32, u32)],
    pub lead: Lead,
    pub drums: Drums,
    pub bass: OscillatorType,
    pub stabs: bool,
    pub moan: bool,
}

pub const LOBBY_TUNES: [LobbyTune; 4] = [
    LobbyTune {
        name: "Graveyard Shuffle",
        bpm: 116.0,
        swing: 0.24,
        chords: [[38, 41, 45, 41], [34, 38, 41, 38], [31, 34, 38, 34], [33, 37, 40, 37]],
        melody: &[
            (0, 0, 74, 3), (0, 4, 77, 2), (0, 6, 76, 2), (0, 8, 74, 4), (0, 14, 69, 2),
            (1, 0, 70, 6), (1, 8, 74, 2), (1, 10, 73, 2), (1, 12, 70, 4),
            (2, 0, 67, 3), (2, 4, 70, 2), (2, 6, 74, 2), (2, 8, 79, 6),
            (3, 0, 76, 3), (3, 4, 73, 3), (3, 8, 69, 8),
            (6, 0, 81, 4), (6, 6, 79, 2), (6, 8, 77, 4), (7, 0, 76, 4), (7, 4, 73, 4), (7, 8, 74, 8),
        ],
        lead: Lead::Theremin,
        drums: Drums::Shuffle,
        bass: OscillatorType::Sawtooth,
        stabs: true,
        moan: true,
    },
    LobbyTune {
        name: "Midnight Bloom",
        bpm: 84.0,
        swing: 0.0,
        chords: [[33, 36, 40, 36], [29, 33, 36, 33], [36, 40, 43, 40], [31, 35, 38, 35]],
        melody: &[
            (0, 0, 76, 4), (0, 4, 74, 2), (0, 6, 72, 2), (0, 8, 69, 8),
            (1, 0, 72, 4), (1, 4, 74, 4), (1, 8, 76, 8),
            (2, 0, 79, 6), (2, 8, 77, 4), (2, 12, 76, 4),
            (3, 0, 74, 12),
            (4, 0, 81, 4), (4, 4, 79, 4), (4, 8, 76, 8),
            (5, 0, 77, 4), (5, 4, 76, 4), (5, 8, 72, 8),
            (6, 0, 74, 4), (6, 4, 72, 4), (6, 8, 71, 8), (7, 0, 69, 16),
        ],
        lead: Lead::Bell,
        drums: Drums::Halftime,
        bass: OscillatorType::Sine,
        stabs: false,
        moan: true,
    },
    LobbyTune {
        name: "Dead Man's Disco",
        bpm: 124.0,
        swing: 0.0,
        chords: [[40, 43, 47, 43], [36, 40, 43, 40], [43, 47, 50, 47], [38, 42, 45, 42]],
        melody: &[
            (0, 0, 76, 2), (0, 2, 76, 2), (0, 6, 79, 2), (0, 8, 76, 2), (0, 12, 74, 4),
            (1, 0, 72, 2), (1, 2, 72, 2), (1, 6, 76, 2), (1, 8, 72, 2), (1, 12, 71, 4),
            (2, 0, 79, 2), (2, 2, 79, 2), (2, 6, 83, 2), (2, 8, 79, 2), (2, 12, 78, 4),
            (3, 0, 74, 4), (3, 4, 76, 4), (3, 8, 78, 8),
            (6, 0, 88, 2), (6, 4, 86, 2), (6, 8, 83, 4), (7, 0, 79, 4), (7, 8, 76, 8),
        ],
        lead: Lead::Square,
        drums: Drums::Disco,
        bass: OscillatorType::Square,
        stabs: true,
        moan: false,
    },
    LobbyTune {
        name: "Lighthouse Lullaby",
        bpm: 72.0,
        swing: 0.08,
        chords: [[36, 40, 43, 40], [33, 36, 40, 36], [29, 33, 36, 33], [31, 35, 38, 35]],
        melody: &[
            (0, 0, 72, 6), (0, 8, 76, 4), (0, 12, 79, 4),
            (1, 0, 81, 8), (1, 8, 79, 8),
            (2, 0, 77, 6), (2, 8, 76, 4), (2, 12, 74, 4),
            (3, 0, 74, 16),
            (4, 0, 72, 6), (4, 8, 76, 4), (4, 12, 79, 4),
            (5, 0, 84, 8), (5, 8, 83, 8), (6, 0, 81, 6), (6, 8, 77, 8), (7, 0, 72, 16),
        ],
        lead: Lead::Bell,
        drums: Drums::None,
        bass: OscillatorType::Triangle,
        stabs: false,
        moan: false,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Gameplay,
    /// "Graveyard Shuffle": swung, spooky-fun lobby theme
    Lobby,
}

impl Audio {
    pub fn new() -> Self {
        Audio {
            ctx: None,
            master: None,
            sfx_bus: None,
            music_bus: None,
            stems: None,
            muffle: None,
            low_hp: 0.0,
            next_heart: 0.0,
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
            music_on: true, // lobby theme starts as soon as audio is unlocked
            track: Track::Lobby,
            lobby_tune: 0,
        }
    }

    /// Must be called from a user gesture (browser autoplay policy).
    pub fn unlock(&mut self) {
        // native unit tests have no WebAudio
        if cfg!(not(target_arch = "wasm32")) {
            return;
        }
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

        let muffle = ctx.create_biquad_filter().ok()?;
        muffle.set_type(BiquadFilterType::Lowpass);
        muffle.frequency().set_value(20000.0);
        muffle.connect_with_audio_node(&comp).ok()?;
        let master = ctx.create_gain().ok()?;
        master.gain().set_value(self.volume);
        master.connect_with_audio_node(&muffle).ok()?;

        let sfx = ctx.create_gain().ok()?;
        sfx.gain().set_value(1.0);
        sfx.connect_with_audio_node(&master).ok()?;
        let music = ctx.create_gain().ok()?;
        music.gain().set_value(self.music_volume * 0.55);
        music.connect_with_audio_node(&master).ok()?;
        let mk = |v: f32| -> Option<GainNode> {
            let g = ctx.create_gain().ok()?;
            g.gain().set_value(v);
            g.connect_with_audio_node(&music).ok()?;
            Some(g)
        };
        let stems = [mk(1.0)?, mk(0.0)?, mk(0.0)?];

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
        self.stems = Some(stems);
        self.muffle = Some(muffle);
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
                | Sfx::ReadyUp
                | Sfx::MatchStart
                | Sfx::Stinger
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
            Sfx::Stinger => {
                // a bright rising triad
                for (k, f) in [523.25f32, 659.25, 783.99, 1046.5].iter().enumerate() {
                    let tt = t + k as f64 * 0.06;
                    self.tone(&d, O::Square, tt, *f, *f, 0.18, 0.06 * v, 0.004)?;
                    self.tone(&d, O::Triangle, tt, *f * 2.0, *f * 2.0, 0.22, 0.04 * v, 0.004)?;
                }
            }
            Sfx::ShootHeavy => {
                // transient, body, and a long low tail
                self.noise(&d, t, F::Highpass, 5000.0, 2500.0, 0.7, 0.03, 0.25 * v)?;
                self.noise(&d, t + 0.02, F::Lowpass, 900.0, 120.0, 0.7, 0.55, 0.12 * v)?;
                self.noise(&d, t, F::Lowpass, 2600.0 * jit, 150.0, 1.0, 0.28, 0.55 * v)?;
                self.tone(&d, O::Sine, t, 140.0, 38.0, 0.25, 0.5 * v, 0.002)?;
                self.tone(&d, O::Square, t, 420.0 * jit, 90.0, 0.08, 0.06 * v, 0.002)?;
            }
            Sfx::Shoot => {
                self.noise(&d, t + 0.015, F::Lowpass, 1400.0, 200.0, 0.7, 0.22, 0.05 * v)?;
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
            Sfx::ReadyUp => {
                // bright rising major arpeggio with a sparkle on top
                for (k, f) in [523.25f32, 659.25, 783.99, 1046.5].iter().enumerate() {
                    let tt = t + k as f64 * 0.075;
                    self.tone(&d, O::Triangle, tt, *f, *f, 0.32, 0.12, 0.004)?;
                    self.tone(&d, O::Square, tt, *f * 2.0, *f * 2.0, 0.08, 0.025, 0.002)?;
                }
                self.noise(&d, t + 0.3, F::Highpass, 6000.0, 9000.0, 0.7, 0.35, 0.05)?;
            }
            Sfx::MatchStart => {
                // whoosh into a heavy power chord stab, then a rising call
                self.noise(&d, t, F::Bandpass, 300.0, 4000.0, 1.2, 0.45, 0.22)?;
                let hit = t + 0.38;
                for f in [73.42f32, 110.0, 146.83, 220.0] {
                    self.tone(&d, O::Sawtooth, hit, f, f * 0.97, 0.9, 0.11, 0.006)?;
                }
                self.tone(&d, O::Sine, hit, 90.0, 40.0, 0.5, 0.6, 0.003)?;
                self.noise(&d, hit, F::Lowpass, 1800.0, 120.0, 0.8, 0.5, 0.35)?;
                for (k, f) in [293.66f32, 369.99, 440.0, 587.33].iter().enumerate() {
                    let tt = hit + 0.12 + k as f64 * 0.09;
                    self.tone(&d, O::Square, tt, *f, *f, 0.22, 0.05, 0.003)?;
                }
            }
        }
        Some(())
    }

    /// Lookahead-scheduled procedural soundtrack. `intensity` 0..1 drives tempo/layers.
    pub fn set_track(&mut self, track: Track) {
        if self.track != track {
            self.track = track;
            self.beat = 0;
            self.next_beat = self.now() + 0.15;
        }
    }

    /// Sine "theremin" voice with vibrato, for the lobby melody.
    fn theremin(&self, dest: &AudioNode, t: f64, freq: f32, dur: f64, gain: f32) -> Option<()> {
        let ctx = self.ctx.as_ref()?;
        let o = ctx.create_oscillator().ok()?;
        o.set_type(OscillatorType::Sine);
        // slight upward scoop into the note, theremin style
        o.frequency().set_value_at_time(freq * 0.94, t).ok()?;
        o.frequency()
            .exponential_ramp_to_value_at_time(freq, t + 0.06)
            .ok()?;
        let lfo = ctx.create_oscillator().ok()?;
        lfo.frequency().set_value(5.8);
        let depth = ctx.create_gain().ok()?;
        depth.gain().set_value(freq * 0.018);
        lfo.connect_with_audio_node(&depth).ok()?;
        depth.connect_with_audio_param(&o.frequency()).ok()?;
        let g = ctx.create_gain().ok()?;
        g.gain().set_value_at_time(0.0001, t).ok()?;
        g.gain()
            .exponential_ramp_to_value_at_time(gain, t + 0.05)
            .ok()?;
        g.gain().set_value_at_time(gain, t + dur * 0.7).ok()?;
        g.gain()
            .exponential_ramp_to_value_at_time(0.0001, t + dur)
            .ok()?;
        o.connect_with_audio_node(&g).ok()?;
        g.connect_with_audio_node(dest).ok()?;
        o.start_with_when(t).ok()?;
        lfo.start_with_when(t).ok()?;
        o.stop_with_when(t + dur + 0.05).ok()?;
        lfo.stop_with_when(t + dur + 0.05).ok()?;
        Some(())
    }

    /// Lobby theme: a swung walking bass, organ offbeats, a wobbly theremin
    /// melody and the occasional zombie moan. 16 steps per bar (16th notes,
    /// swung), 8-bar loop over i - VI - iv - V in D minor.
    fn lobby_step(&mut self, bus: &AudioNode, t: f64, sixteenth: f64) -> Option<()> {
        use BiquadFilterType as F;
        use OscillatorType as O;
        let tune = &LOBBY_TUNES[self.lobby_tune % LOBBY_TUNES.len()];
        let mtof = |m: i32| 440.0 * 2f32.powf((m - 69) as f32 / 12.0);
        let b = self.beat;
        let st = b % 16;
        let bar = (b / 16) % 8;
        let ch = tune.chords[(bar % 4) as usize];
        // bass on each beat (walking through the chord)
        if st % 4 == 0 {
            let n = ch[(st / 4) as usize];
            let f = mtof(n);
            self.tone(bus, O::Triangle, t, f, f, sixteenth * 3.2, 0.32, 0.005)?;
            self.tone(bus, tune.bass, t, f, f * 0.995, sixteenth * 1.6, 0.05, 0.005)?;
        }
        // chord stabs on the offbeats
        if tune.stabs && st % 4 == 2 {
            for (k, n) in [ch[0] + 24, ch[1] + 24, ch[2] + 24].iter().enumerate() {
                let f = mtof(*n);
                let det = 1.0 + (k as f32 - 1.0) * 0.004;
                self.tone(bus, O::Square, t, f * det, f * det, sixteenth * 1.4, 0.022, 0.004)?;
            }
        }
        // drums
        match tune.drums {
            Drums::Shuffle | Drums::Disco => {
                let four = tune.drums == Drums::Disco;
                if (four && st % 4 == 0) || (!four && (st == 0 || st == 8 || (bar % 2 == 1 && st == 14))) {
                    self.tone(bus, O::Sine, t, 140.0, 42.0, 0.16, 0.5, 0.003)?;
                }
                if st == 4 || st == 12 {
                    self.noise(bus, t, F::Bandpass, 1600.0, 1200.0, 1.2, 0.12, 0.2)?;
                    self.noise(bus, t + 0.012, F::Bandpass, 2100.0, 1500.0, 1.5, 0.08, 0.12)?;
                }
                if st % 2 == 0 || (four && st % 2 == 1) {
                    let open = if four { st % 4 == 2 } else { st % 8 == 6 };
                    self.noise(bus, t, F::Highpass, 8000.0, 8000.0, 0.7, if open { 0.12 } else { 0.035 }, 0.05)?;
                }
            }
            Drums::Halftime => {
                if st == 0 {
                    self.tone(bus, O::Sine, t, 120.0, 40.0, 0.25, 0.45, 0.003)?;
                }
                if st == 8 {
                    self.noise(bus, t, F::Bandpass, 1400.0, 900.0, 1.0, 0.25, 0.16)?;
                }
                if st % 4 == 2 {
                    self.noise(bus, t, F::Highpass, 9000.0, 9000.0, 0.7, 0.03, 0.03)?;
                }
            }
            Drums::None => {}
        }
        // melody: (bar, step, midi note, length in 16ths)
        for (mb, ms, note, len) in tune.melody.iter() {
            if *mb == bar && *ms == st {
                let dur = sixteenth * *len as f64 * 1.05;
                match tune.lead {
                    Lead::Theremin => self.theremin(bus, t, mtof(*note), dur, 0.075)?,
                    Lead::Bell => {
                        let f = mtof(*note);
                        self.tone(bus, O::Sine, t, f, f, dur * 1.6, 0.07, 0.003)?;
                        self.tone(bus, O::Sine, t, f * 2.76, f * 2.76, dur * 0.6, 0.02, 0.002)?;
                    }
                    Lead::Square => {
                        let f = mtof(*note);
                        self.tone(bus, O::Square, t, f, f, dur, 0.035, 0.004)?;
                    }
                }
            }
        }
        if tune.moan && bar == 7 && st == 12 {
            // zombie moan pickup at the end of the loop
            self.tone(bus, O::Sawtooth, t, 110.0, 70.0, 0.8, 0.06, 0.15)?;
            self.noise(bus, t, F::Bandpass, 520.0, 300.0, 9.0, 0.8, 0.07)?;
        }
        Some(())
    }


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
        if self.track == Track::Lobby {
            let tune = &LOBBY_TUNES[self.lobby_tune % LOBBY_TUNES.len()];
            let sixteenth = 60.0 / tune.bpm / 4.0;
            let swing_amt = tune.swing;
            while self.next_beat < now + 0.2 {
                let t = self.next_beat;
                self.lobby_step(&bus, t, sixteenth)?;
                // swing: long-short pairs of 16ths
                let swing = if self.beat % 2 == 0 { 1.0 + swing_amt } else { 1.0 - swing_amt };
                self.next_beat += sixteenth * swing;
                self.beat = self.beat.wrapping_add(1);
            }
            return Some(());
        }
        // crossfade stems and the low-health muffle
        let it = self.intensity;
        let smooth = |a: f32, b: f32| ((it - a) / (b - a)).clamp(0.0, 1.0);
        if let Some(st) = &self.stems {
            let _ = st[1].gain().set_target_at_time(smooth(0.12, 0.45), now, 0.4);
            let _ = st[2].gain().set_target_at_time(smooth(0.4, 0.8), now, 0.6);
        }
        if let Some(m) = &self.muffle {
            let f = 20000.0 * (1.0 - self.low_hp) + 700.0 * self.low_hp;
            let _ = m.frequency().set_target_at_time(f, now, 0.25);
        }
        if self.low_hp > 0.05 && self.next_heart < now + 0.2 {
            let t = self.next_heart.max(now + 0.02);
            let sfx: AudioNode = self.sfx_bus.as_ref()?.clone().into();
            let g = 0.35 + 0.4 * self.low_hp;
            self.tone(&sfx, OscillatorType::Sine, t, 70.0, 38.0, 0.16, g, 0.004)?;
            self.tone(&sfx, OscillatorType::Sine, t + 0.18, 64.0, 34.0, 0.14, g * 0.7, 0.004)?;
            self.next_heart = t + 0.95 - 0.35 * self.low_hp as f64;
        }
        let stems: [AudioNode; 3] = match &self.stems {
            Some(s) => [s[0].clone().into(), s[1].clone().into(), s[2].clone().into()],
            None => [bus.clone(), bus.clone(), bus.clone()],
        };
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
                self.tone(&stems[0], O::Sawtooth, t, root, root, step * 1.6, 0.11, 0.01)?;
                self.tone(
                    &stems[0],
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
                self.tone(&stems[0], O::Sine, t, 120.0, 40.0, 0.18, 0.45, 0.003)?;
            }
            // hats (percussion stem, faded in by intensity)
            if st % 2 == 1 {
                self.noise(
                    &stems[1],
                    t,
                    F::Highpass,
                    7000.0,
                    7000.0,
                    0.7,
                    0.04,
                    0.05 + 0.05 * self.intensity,
                )?;
            }
            // snare on 2 & 4
            if st == 4 || st == 12 {
                self.noise(&stems[1], t, F::Bandpass, 1800.0, 900.0, 0.8, 0.14, 0.18)?;
            }
            // driving 16th-note saw arpeggio (lead stem) for the big fights
            if self.intensity > 0.35 {
                let arp = [1.0f32, 1.5, 2.0, 1.189][(st % 4) as usize];
                let f = root * 4.0 * arp;
                self.tone(&stems[2], O::Sawtooth, t, f, f, step * 0.45, 0.028, 0.004)?;
            }
            // eerie lead arpeggio
            if st % 8 == 6 && (b / 8) % 2 == 1 {
                let f = root * 8.0 * [1.0, 1.189, 1.498, 1.335][(b as usize / 16) % 4];
                self.tone(&stems[2], O::Triangle, t, f, f * 0.995, step * 3.0, 0.035, 0.05)?;
            }
            self.next_beat += step;
            self.beat = self.beat.wrapping_add(1);
        }
        Some(())
    }
}
