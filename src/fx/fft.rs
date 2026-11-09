//! A radix-2 complex FFT that never allocates after `new`, so the realtime
//! thread can use it.

use std::f32::consts::TAU;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }

    pub fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }

    pub fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }

    pub fn mul(self, other: Self) -> Self {
        Self::new(self.re * other.re - self.im * other.im, self.re * other.im + self.im * other.re)
    }

    pub fn scale(self, factor: f32) -> Self {
        Self::new(self.re * factor, self.im * factor)
    }

    pub fn norm_sqr(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    pub fn arg(self) -> f32 {
        self.im.atan2(self.re)
    }

    pub fn from_angle(angle: f32) -> Self {
        Self::new(angle.cos(), angle.sin())
    }
}

/// Periodic Hann window, which overlap-adds to a constant.
pub fn hann(size: usize) -> Vec<f32> {
    (0..size).map(|i| 0.5 - 0.5 * (TAU * i as f32 / size as f32).cos()).collect()
}

pub struct Fft {
    bit_reverse: Vec<u32>,
    twiddles: Vec<Complex>,
}

impl Fft {
    /// `size` must be a power of two.
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "FFT size {size}");
        let bits = size.trailing_zeros();
        let bit_reverse = (0..size as u32).map(|i| if bits == 0 { 0 } else { i.reverse_bits() >> (32 - bits) }).collect();
        let twiddles = (0..size / 2).map(|i| Complex::from_angle(-TAU * i as f32 / size as f32)).collect();
        Self { bit_reverse, twiddles }
    }

    pub fn forward(&self, data: &mut [Complex]) {
        let size = data.len();
        debug_assert_eq!(size, self.bit_reverse.len());
        for (i, &reversed) in self.bit_reverse.iter().enumerate() {
            let j = reversed as usize;
            if i < j {
                data.swap(i, j);
            }
        }
        let mut span = 2;
        while span <= size {
            let half = span / 2;
            let step = size / span;
            for block in data.chunks_exact_mut(span) {
                let (low, high) = block.split_at_mut(half);
                for (index, (a, b)) in low.iter_mut().zip(high.iter_mut()).enumerate() {
                    let rotated = b.mul(self.twiddles[index * step]);
                    *b = a.sub(rotated);
                    *a = a.add(rotated);
                }
            }
            span *= 2;
        }
    }

    /// Unscaled: forward then inverse multiplies the data by its length.
    pub fn inverse(&self, data: &mut [Complex]) {
        data.iter_mut().for_each(|value| *value = Complex::new(value.im, value.re));
        self.forward(data);
        data.iter_mut().for_each(|value| *value = Complex::new(value.im, value.re));
    }
}

pub fn self_test() -> Result<(), String> {
    let fft = Fft::new(64);
    let mut data: Vec<Complex> = (0..64).map(|i| Complex::from_angle(TAU * 5.0 * i as f32 / 64.0)).collect();
    let original = data.clone();
    fft.forward(&mut data);
    if (data[5].re - 64.0).abs() > 0.01 || data[6].norm_sqr() > 0.01 {
        return Err(format!("fft bin {:?}", data[5]));
    }
    fft.inverse(&mut data);
    let wrong = data.iter().zip(&original).any(|(a, b)| (a.re / 64.0 - b.re).abs() > 1e-3 || (a.im / 64.0 - b.im).abs() > 1e-3);
    if wrong {
        return Err("fft round trip".into());
    }
    Ok(())
}
