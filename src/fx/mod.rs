pub mod analyzer;
pub mod dsp;
mod fft;
mod pitch;
pub mod presets;
mod reverb;

pub fn self_test() -> Result<(), String> {
    fft::self_test()?;
    analyzer::self_test()?;
    presets::self_test()?;
    dsp::self_test()?;
    Ok(())
}
