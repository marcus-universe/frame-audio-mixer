pub mod dsp;

pub fn self_test() -> Result<(), String> {
    dsp::self_test()?;
    Ok(())
}
