//! Opus, one stereo 10 ms frame at a time.
//!
//! `unsafe-libopus` is the reference C codec transpiled to Rust, so it builds
//! on Windows, Linux and the headset without a C toolchain. Its API is the C
//! one; these two types are the only place that touches it.

use std::ptr::NonNull;

use unsafe_libopus::{
    opus_decode_float, opus_decoder_create, opus_decoder_ctl, opus_decoder_destroy, opus_encode_float, opus_encoder_create,
    opus_encoder_ctl, opus_encoder_destroy, OpusDecoder, OpusEncoder, OPUS_APPLICATION_AUDIO, OPUS_OK, OPUS_RESET_STATE,
    OPUS_SET_BITRATE_REQUEST, OPUS_SET_COMPLEXITY_REQUEST, OPUS_SET_VBR_CONSTRAINT_REQUEST, OPUS_SET_VBR_REQUEST,
};

use crate::{CHANNELS, FRAME, FRAME_SAMPLES, RATE};

/// Largest coded frame this link sends. 510 kbit/s, Opus's ceiling, is 638
/// bytes in 10 ms.
pub(crate) const MAX_PACKET: usize = 640;

/// The codec refused a call. The number is libopus's own error code.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("the Opus encoder could not be created (code {0})")]
    EncoderCreate(i32),
    #[error("the Opus decoder could not be created (code {0})")]
    DecoderCreate(i32),
    #[error("Opus rejected an encoder setting (code {0})")]
    EncoderSetting(i32),
    #[error("Opus could not encode a frame (code {0})")]
    Encode(i32),
    #[error("Opus could not decode a frame (code {0})")]
    Decode(i32),
}

pub(crate) struct Encoder {
    raw: NonNull<OpusEncoder>,
}

// SAFETY: the encoder state is plain heap memory with no thread affinity, and
// `&mut self` on every method keeps it on one thread at a time.
unsafe impl Send for Encoder {}

impl Encoder {
    /// `bitrate` is in bits per second for both channels together. `complexity`
    /// is Opus's 0–10 effort setting.
    pub(crate) fn new(bitrate: i32, complexity: i32) -> Result<Self, CodecError> {
        let mut code = OPUS_OK;
        // SAFETY: plain constructor; `code` outlives the call.
        let raw = unsafe { opus_encoder_create(RATE as i32, CHANNELS as i32, OPUS_APPLICATION_AUDIO, &mut code) };
        let raw = NonNull::new(raw).filter(|_| code == OPUS_OK).ok_or(CodecError::EncoderCreate(code))?;
        let encoder = Self { raw };
        encoder.set(OPUS_SET_BITRATE_REQUEST, bitrate)?;
        encoder.set(OPUS_SET_COMPLEXITY_REQUEST, complexity)?;
        // Variable rate, so silence and quiet passages cost almost nothing, but
        // constrained, so a loud passage cannot outgrow one datagram.
        encoder.set(OPUS_SET_VBR_REQUEST, 1)?;
        encoder.set(OPUS_SET_VBR_CONSTRAINT_REQUEST, 1)?;
        Ok(encoder)
    }

    fn set(&self, request: i32, value: i32) -> Result<(), CodecError> {
        // SAFETY: `raw` is a live encoder and every request used here takes one i32.
        let code = unsafe { opus_encoder_ctl!(self.raw.as_ptr(), request, value) };
        if code == OPUS_OK {
            Ok(())
        } else {
            Err(CodecError::EncoderSetting(code))
        }
    }

    /// Forgets everything heard so far. The next frame decodes without history.
    pub(crate) fn reset(&mut self) {
        // SAFETY: `raw` is a live encoder; this request takes no argument.
        unsafe { opus_encoder_ctl!(self.raw.as_ptr(), OPUS_RESET_STATE) };
    }

    /// Codes one frame into `out` and returns how many bytes it took.
    pub(crate) fn encode(&mut self, pcm: &[f32; FRAME_SAMPLES], out: &mut [u8; MAX_PACKET]) -> Result<usize, CodecError> {
        // SAFETY: `pcm` holds exactly FRAME stereo frames and `out` MAX_PACKET bytes.
        let written = unsafe { opus_encode_float(self.raw.as_ptr(), pcm.as_ptr(), FRAME as i32, out.as_mut_ptr(), MAX_PACKET as i32) };
        usize::try_from(written).map_err(|_| CodecError::Encode(written))
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: created by `opus_encoder_create` and destroyed exactly once.
        unsafe { opus_encoder_destroy(self.raw.as_ptr()) };
    }
}

pub(crate) struct Decoder {
    raw: NonNull<OpusDecoder>,
}

// SAFETY: as for `Encoder`.
unsafe impl Send for Decoder {}

impl Decoder {
    pub(crate) fn new() -> Result<Self, CodecError> {
        let mut code = OPUS_OK;
        // SAFETY: plain constructor; `code` outlives the call.
        let raw = unsafe { opus_decoder_create(RATE as i32, CHANNELS as i32, &mut code) };
        let raw = NonNull::new(raw).filter(|_| code == OPUS_OK).ok_or(CodecError::DecoderCreate(code))?;
        Ok(Self { raw })
    }

    pub(crate) fn reset(&mut self) {
        // SAFETY: `raw` is a live decoder; this request takes no argument.
        unsafe { opus_decoder_ctl!(self.raw.as_ptr(), OPUS_RESET_STATE) };
    }

    /// Decodes one frame. `None` stands for a frame that never arrived: the
    /// decoder then conceals the gap from what it heard before.
    pub(crate) fn decode(&mut self, packet: Option<&[u8]>, out: &mut [f32; FRAME_SAMPLES]) -> Result<(), CodecError> {
        let (data, len) = match packet {
            Some(bytes) => (bytes.as_ptr(), bytes.len() as i32),
            None => (std::ptr::null(), 0),
        };
        // SAFETY: `data` is valid for `len` bytes (or null with 0, which asks for
        // concealment) and `out` has room for FRAME stereo frames.
        let frames = unsafe { opus_decode_float(self.raw.as_ptr(), data, len, out.as_mut_ptr(), FRAME as i32, 0) };
        if frames == FRAME as i32 {
            Ok(())
        } else {
            // A valid packet of another duration is not one of ours.
            out.fill(0.0);
            Err(CodecError::Decode(frames))
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: created by `opus_decoder_create` and destroyed exactly once.
        unsafe { opus_decoder_destroy(self.raw.as_ptr()) };
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// One frame of a sine on both channels, continuing at `phase`.
    pub(crate) fn sine_frame(phase: &mut f64, hz: f64, level: f32) -> [f32; FRAME_SAMPLES] {
        let mut frame = [0.0; FRAME_SAMPLES];
        for pair in frame.as_chunks_mut::<CHANNELS>().0 {
            let sample = (*phase).sin() as f32 * level;
            pair[0] = sample;
            pair[1] = sample;
            *phase += std::f64::consts::TAU * hz / f64::from(RATE);
        }
        frame
    }

    pub(crate) fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    #[test]
    fn round_trip_should_keep_the_level_of_a_tone() -> Result<(), CodecError> {
        let mut encoder = Encoder::new(128_000, 5)?;
        let mut decoder = Decoder::new()?;
        let mut packet = [0u8; MAX_PACKET];
        let mut out = [0.0; FRAME_SAMPLES];
        let mut phase = 0.0;
        // The codec needs a few frames to settle; judge the last one.
        for _ in 0..20 {
            let frame = sine_frame(&mut phase, 440.0, 0.5);
            let len = encoder.encode(&frame, &mut packet)?;
            decoder.decode(Some(&packet[..len]), &mut out)?;
        }

        let expected = 0.5 / std::f32::consts::SQRT_2;
        assert!((rms(&out) - expected).abs() < 0.02, "rms {}", rms(&out));
        Ok(())
    }

    #[test]
    fn silence_should_cost_a_few_bytes() -> Result<(), CodecError> {
        let mut encoder = Encoder::new(128_000, 5)?;
        let mut packet = [0u8; MAX_PACKET];
        let mut len = 0;
        for _ in 0..10 {
            len = encoder.encode(&[0.0; FRAME_SAMPLES], &mut packet)?;
        }

        assert!(len <= 8, "silent frame took {len} bytes");
        Ok(())
    }

    #[test]
    fn missing_frame_should_be_concealed_not_silent() -> Result<(), CodecError> {
        let mut encoder = Encoder::new(128_000, 5)?;
        let mut decoder = Decoder::new()?;
        let mut packet = [0u8; MAX_PACKET];
        let mut out = [0.0; FRAME_SAMPLES];
        let mut phase = 0.0;
        for _ in 0..20 {
            let frame = sine_frame(&mut phase, 440.0, 0.5);
            let len = encoder.encode(&frame, &mut packet)?;
            decoder.decode(Some(&packet[..len]), &mut out)?;
        }
        decoder.decode(None, &mut out)?;

        assert!(rms(&out) > 0.1, "concealed frame rms {}", rms(&out));
        Ok(())
    }

    #[test]
    fn garbage_should_be_rejected_and_leave_silence() -> Result<(), CodecError> {
        let mut decoder = Decoder::new()?;
        let mut out = [1.0; FRAME_SAMPLES];
        // A valid 20 ms SILK frame header: the wrong duration for this link.
        let result = decoder.decode(Some(&[0x08, 0xff, 0xfe]), &mut out);

        assert!(result.is_err());
        assert!(out.iter().all(|sample| *sample == 0.0));
        Ok(())
    }
}
