use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);

    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn len2(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            v2(self.x / l, self.y / l)
        } else {
            V2::ZERO
        }
    }
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn perp(self) -> V2 {
        v2(-self.y, self.x)
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn from_angle(a: f32) -> V2 {
        v2(a.cos(), a.sin())
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        v2(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn dist2(self, o: V2) -> f32 {
        (self - o).len2()
    }
    pub fn clamp_len(self, max: f32) -> V2 {
        let l = self.len();
        if l > max {
            self * (max / l)
        } else {
            self
        }
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        v2(self.x * s, self.y * s)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl MulAssign<f32> for V2 {
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
    }
}

/// Small, fast xorshift RNG (deterministic, no deps).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }
    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n.max(1)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
    pub fn angle(&mut self) -> f32 {
        self.f() * std::f32::consts::TAU
    }
    pub fn unit(&mut self) -> V2 {
        V2::from_angle(self.angle())
    }
}

/// Stateless integer hash used for the infinite procedural world.
pub fn hash2(x: i32, y: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

pub fn hashf(x: i32, y: i32, salt: u32) -> f32 {
    (hash2(x, y, salt) >> 8) as f32 / (1u32 << 24) as f32
}

/// Frame-rate independent exponential smoothing factor.
pub fn damp(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Wrap an angle difference into [-PI, PI].
pub fn wrap_angle(mut a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    while a > PI {
        a -= TAU;
    }
    while a < -PI {
        a += TAU;
    }
    a
}
