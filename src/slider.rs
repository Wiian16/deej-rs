use std::collections::VecDeque;

use crate::{audio::NormalizedVolume, config::NoiseReduction};

/// One line's worth of normalized slider readings, in serial order.
pub type SliderFrame = Vec<NormalizedVolume>;

/// Parses a raw serial line like `401|410|517|561|612` into normalized slider values.
/// `max_value` is the firmware's max raw ADC reading (1023, for a 10-bit Arduino ADC).
/// Returns 'None' for lines that don't look like a slider frame (power-up, noise, stray
/// firmware debug output, etc.) rather than erroring.
pub fn parse_line(line: &str, max_value: u16, invert: bool) -> Option<SliderFrame> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    line.split('|')
        .map(|part| {
            let raw: u16 = part.trim().parse().ok()?;
            let normalized =
                NormalizedVolume::clamped(f32::from(raw.min(max_value)) / f32::from(max_value));

            Some(if invert {
                NormalizedVolume::clamped(1.0 - normalized.get())
            } else {
                normalized
            })
        })
        .collect()
}

/// Smooths noisy analog readings with a moving average and suppresses updates too small to matter.
/// `window` controls the moving-average size; `epsilon` is the minimum change (in normalized units)
/// required before a slider is reported as moved.
pub struct SliderSmoother {
    window: usize,
    epsilon: f32,
    history: Vec<VecDeque<f32>>,
    last_emitted: Vec<Option<NormalizedVolume>>,
}

impl SliderSmoother {
    pub fn new(reduction: NoiseReduction) -> Self {
        let (window, epsilon) = match reduction {
            NoiseReduction::Low => (4, 0.003),
            NoiseReduction::Default => (8, 0.01),
            NoiseReduction::High => (16, 0.02),
        };

        Self {
            window,
            epsilon,
            history: Vec::new(),
            last_emitted: Vec::new(),
        }
    }

    /// Feeds a raw frame in, returns `(index, value)` for sliders that moved enough
    /// to be worth acting on. Frame length may grow between calls (e.g. firs frame
    /// arrives before we know the slider count); it should not shrink.
    pub fn update(&mut self, frame: &SliderFrame) -> Vec<(usize, NormalizedVolume)> {
        if self.history.len() < frame.len() {
            self.history
                .resize_with(frame.len(), || VecDeque::with_capacity(self.window));
            self.last_emitted.resize(frame.len(), None);
        }

        let mut changed = Vec::new();
        for (i, &raw) in frame.iter().enumerate() {
            let buf = &mut self.history[i];
            if buf.len() == self.window {
                buf.pop_front();
            }
            buf.push_back(raw.get());

            let smoothed = NormalizedVolume::clamped(buf.iter().sum::<f32>() / buf.len() as f32);
            let moved = match self.last_emitted[i] {
                None => true,
                Some(prev) => (smoothed.get() - prev.get()).abs() > self.epsilon,
            };

            if moved {
                self.last_emitted[i] = Some(smoothed);
                changed.push((i, smoothed));
            }
        }

        changed
    }
}

#[cfg(test)]
mod smoother_tests {
    use super::*;

    fn frame(vals: &[f32]) -> SliderFrame {
        vals.iter().map(|&v| NormalizedVolume::clamped(v)).collect()
    }

    #[test]
    fn first_frame_always_emits() {
        let mut s = SliderSmoother::new(NoiseReduction::Default);
        let changed = s.update(&frame(&[0.5, 0.5]));
        assert_eq!(changed.len(), 2);
    }

    #[test]
    fn tiny_jitter_is_suppressed() {
        let mut s = SliderSmoother::new(NoiseReduction::Default);
        s.update(&frame(&[0.5]));
        let changed = s.update(&frame(&[0.501]));
        assert!(
            changed.is_empty(),
            "sub-epsilon change should not be reported"
        );
    }

    #[test]
    fn real_movement_emits() {
        let mut s = SliderSmoother::new(NoiseReduction::Default);
        s.update(&frame(&[0.2]));
        for _ in 0..8 {
            s.update(&frame(&[0.8])); // fill the averaging window
        }
        let changed = s.update(&frame(&[0.8]));
        assert!(changed.is_empty()); // already converged and reported by now
    }
}
