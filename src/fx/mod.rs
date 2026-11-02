mod dsp;
mod engine;

pub use engine::{Engine, InstanceKey};

pub fn self_test() -> Result<(), String> {
    dsp::self_test()?;
    engine::self_test()?;
    Ok(())
}
