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
