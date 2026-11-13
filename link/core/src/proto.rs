//! What goes on the wire.
//!
//! Every datagram starts with a 12-byte header: the magic `FAL1`, the packet
//! kind, three reserved bytes, and the session id (little-endian). Control
//! packets carry one JSON object after it, so a newer peer can add fields an
//! older one ignores. Audio packets carry coded frames back to back.

use serde::{Deserialize, Serialize};

use crate::{Mask, STREAMS};

/// UDP port the headset listens on.
pub const DEFAULT_PORT: u16 = 47_811;
/// Bumped when two ends could no longer understand each other.
pub const PROTOCOL_VERSION: u16 = 1;

const MAGIC: [u8; 4] = *b"FAL1";
const HEADER: usize = 12;
/// Stream, flags, payload length, sequence number.
const BLOB_HEADER: usize = 8;
/// Largest datagram this link sends. With the IP and UDP headers it stays
/// under a 1500-byte MTU, so it is never fragmented.
pub(crate) const MAX_DATAGRAM: usize = 1400;

const KIND_HELLO: u8 = 1;
const KIND_WELCOME: u8 = 2;
const KIND_REFUSE: u8 = 3;
const KIND_STATE: u8 = 4;
const KIND_AUDIO: u8 = 5;
const KIND_BYE: u8 = 6;

/// Set on the first frame after a stream was silent. The receiver starts over:
/// it forgets its buffer and decodes this frame without history.
pub(crate) const BLOB_START: u8 = 1;
/// Set on a second copy of a frame that already went out in the packet before.
/// One lost packet then costs nothing.
pub(crate) const BLOB_REPEAT: u8 = 2;
/// Set on an empty frame that closes a run: the sender went silent and sends
/// nothing more until the next start. Without it the receiver could not tell
/// a sound that ended from a network that stalled.
pub(crate) const BLOB_END: u8 = 4;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum ProtoError {
    #[error("not a link packet")]
    NotOurs,
    #[error("unknown packet kind {0}")]
    UnknownKind(u8),
    #[error("unreadable control packet")]
    BadBody,
}

/// The PC looking for the headset. Sent to every address it might be at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Hello {
    pub v: u16,
    /// Random per run of the PC app. Tells the headset whether two packets
    /// from different addresses are the same PC on two networks.
    pub host: u64,
    pub name: String,
    /// Echoed in the answer, so the PC can time the round trip.
    pub nonce: u64,
}

/// The headset's answer to a `Hello` it accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Welcome {
    pub v: u16,
    pub name: String,
    pub session: u32,
    pub nonce: u64,
}

/// The headset's answer to a `Hello` it does not accept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Refuse {
    pub reason: String,
    #[serde(default)]
    pub nonce: u64,
}

pub(crate) const REFUSE_BUSY: &str = "busy";
pub(crate) const REFUSE_VERSION: &str = "version";
pub(crate) const REFUSE_NOT_ALLOWED: &str = "not-allowed";

/// What one end is willing to do, sent twice a second and on every change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct State {
    /// Streams this end would send.
    pub send: Mask,
    /// Streams this end wants to receive.
    pub recv: Mask,
    /// Streams this end is sending right now (not silent).
    #[serde(default)]
    pub active: Mask,
    /// Display names of the streams this end sends. The headset names B1–B4.
    #[serde(default)]
    pub names: Vec<String>,
}

/// One coded frame inside an audio packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Blob<'a> {
    pub stream: usize,
    pub flags: u8,
    pub seq: u32,
    pub payload: &'a [u8],
}

impl Blob<'_> {
    pub(crate) fn wire_len(&self) -> usize {
        BLOB_HEADER + self.payload.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Packet<'a> {
    Hello(Hello),
    Welcome(Welcome),
    Refuse(Refuse),
    State(State),
    Audio(Blobs<'a>),
    Bye,
}

/// The coded frames of one audio packet, in wire order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Blobs<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Blobs<'a> {
    type Item = Blob<'a>;

    /// A blob that claims more bytes than the packet has ends the iteration,
    /// as does one for a stream that does not exist.
    fn next(&mut self) -> Option<Blob<'a>> {
        let (head, tail) = self.rest.split_at_checked(BLOB_HEADER)?;
        let stream = usize::from(head[0]);
        let len = usize::from(u16::from_le_bytes([head[2], head[3]]));
        let seq = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        let (payload, rest) = tail.split_at_checked(len)?;
        if stream >= STREAMS {
            self.rest = &[];
            return None;
        }
        self.rest = rest;
        Some(Blob { stream, flags: head[1], seq, payload })
    }
}

/// Splits a datagram into its session id and its content.
pub(crate) fn parse(datagram: &[u8]) -> Result<(u32, Packet<'_>), ProtoError> {
    let (head, body) = datagram.split_at_checked(HEADER).ok_or(ProtoError::NotOurs)?;
    if head[..4] != MAGIC {
        return Err(ProtoError::NotOurs);
    }
    let session = u32::from_le_bytes([head[8], head[9], head[10], head[11]]);
    let packet = match head[4] {
        KIND_HELLO => Packet::Hello(json(body)?),
        KIND_WELCOME => Packet::Welcome(json(body)?),
        KIND_REFUSE => Packet::Refuse(json(body)?),
        KIND_STATE => Packet::State(json(body)?),
        KIND_AUDIO => Packet::Audio(Blobs { rest: body }),
        KIND_BYE => Packet::Bye,
        other => return Err(ProtoError::UnknownKind(other)),
    };
    Ok((session, packet))
}

fn json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, ProtoError> {
    serde_json::from_slice(body).map_err(|_| ProtoError::BadBody)
}

fn start(out: &mut Vec<u8>, kind: u8, session: u32) {
    out.clear();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&[kind, 0, 0, 0]);
    out.extend_from_slice(&session.to_le_bytes());
}

fn control<T: Serialize>(out: &mut Vec<u8>, kind: u8, session: u32, body: &T) {
    start(out, kind, session);
    // Serializing these plain structs cannot fail; an empty body would simply
    // be rejected by the receiver.
    if let Ok(json) = serde_json::to_vec(body) {
        out.extend_from_slice(&json);
    }
}

pub(crate) fn write_hello(out: &mut Vec<u8>, hello: &Hello) {
    control(out, KIND_HELLO, 0, hello);
}

pub(crate) fn write_welcome(out: &mut Vec<u8>, welcome: &Welcome) {
    control(out, KIND_WELCOME, welcome.session, welcome);
}

pub(crate) fn write_refuse(out: &mut Vec<u8>, refuse: &Refuse) {
    control(out, KIND_REFUSE, 0, refuse);
}

pub(crate) fn write_state(out: &mut Vec<u8>, session: u32, state: &State) {
    control(out, KIND_STATE, session, state);
}

pub(crate) fn write_bye(out: &mut Vec<u8>, session: u32) {
    start(out, KIND_BYE, session);
}

/// Starts an audio packet in `out`. Frames are added with `audio_push` for as
/// long as `audio_fits` says so.
pub(crate) fn audio_begin(out: &mut Vec<u8>, session: u32) {
    start(out, KIND_AUDIO, session);
}

pub(crate) fn audio_fits(out: &[u8], blob: &Blob<'_>) -> bool {
    out.len() + blob.wire_len() <= MAX_DATAGRAM
}

/// True while the packet started in `out` holds no frame yet.
pub(crate) fn audio_is_empty(out: &[u8]) -> bool {
    out.len() <= HEADER
}

/// Appends a frame. The caller checks `audio_fits` first; a frame longer than
/// a u16 can describe is not something the encoder produces.
pub(crate) fn audio_push(out: &mut Vec<u8>, blob: &Blob<'_>) {
    let Ok(len) = u16::try_from(blob.payload.len()) else { return };
    // `stream` is below STREAMS, so it fits a byte.
    out.push(blob.stream as u8);
    out.push(blob.flags);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&blob.seq.to_le_bytes());
    out.extend_from_slice(blob.payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_should_survive_the_wire() {
        let hello = Hello { v: PROTOCOL_VERSION, host: 7, name: "PC".into(), nonce: 99 };
        let mut wire = Vec::new();
        write_hello(&mut wire, &hello);

        assert_eq!(parse(&wire), Ok((0, Packet::Hello(hello))));
    }

    #[test]
    fn state_should_carry_its_session() {
        let state = State { send: 0b0101, recv: 0b1111, active: 0b0001, names: vec!["Discord".into()] };
        let mut wire = Vec::new();
        write_state(&mut wire, 0xdead_beef, &state);

        assert_eq!(parse(&wire), Ok((0xdead_beef, Packet::State(state))));
    }

    #[test]
    fn state_from_a_newer_peer_should_ignore_unknown_fields() {
        let mut wire = Vec::new();
        start(&mut wire, KIND_STATE, 1);
        wire.extend_from_slice(br#"{"send":3,"recv":1,"future":true}"#);

        let expected = State { send: 3, recv: 1, ..State::default() };
        assert_eq!(parse(&wire), Ok((1, Packet::State(expected))));
    }

    #[test]
    fn audio_should_return_every_frame_in_order() {
        let mut wire = Vec::new();
        audio_begin(&mut wire, 5);
        audio_push(&mut wire, &Blob { stream: 2, flags: BLOB_START, seq: 10, payload: &[1, 2, 3] });
        audio_push(&mut wire, &Blob { stream: 0, flags: 0, seq: 70_000, payload: &[] });

        let Ok((5, Packet::Audio(blobs))) = parse(&wire) else { panic!("not an audio packet") };
        let blobs: Vec<_> = blobs.collect();
        assert_eq!(
            blobs,
            vec![
                Blob { stream: 2, flags: BLOB_START, seq: 10, payload: &[1, 2, 3] },
                Blob { stream: 0, flags: 0, seq: 70_000, payload: &[] },
            ]
        );
    }

    #[test]
    fn truncated_audio_should_stop_before_the_broken_frame() {
        let mut wire = Vec::new();
        audio_begin(&mut wire, 5);
        audio_push(&mut wire, &Blob { stream: 1, flags: 0, seq: 1, payload: &[9; 4] });
        audio_push(&mut wire, &Blob { stream: 1, flags: 0, seq: 2, payload: &[9; 40] });
        wire.truncate(wire.len() - 10);

        let Ok((_, Packet::Audio(blobs))) = parse(&wire) else { panic!("not an audio packet") };
        assert_eq!(blobs.count(), 1);
    }

    #[test]
    fn audio_for_a_stream_that_does_not_exist_should_be_dropped() {
        let mut wire = Vec::new();
        start(&mut wire, KIND_AUDIO, 5);
        wire.extend_from_slice(&[STREAMS as u8, 0, 0, 0, 0, 0, 0, 0]);

        let Ok((_, Packet::Audio(mut blobs))) = parse(&wire) else { panic!("not an audio packet") };
        assert_eq!(blobs.next(), None);
    }

    #[test]
    fn foreign_datagram_should_be_rejected() {
        assert_eq!(parse(b"M-SEARCH * HTTP/1.1\r\n"), Err(ProtoError::NotOurs));
    }

    #[test]
    fn short_datagram_should_be_rejected() {
        assert_eq!(parse(b"FAL1"), Err(ProtoError::NotOurs));
    }

    #[test]
    fn packet_should_refuse_a_frame_past_the_datagram_limit() {
        let mut wire = Vec::new();
        audio_begin(&mut wire, 5);
        let payload = [0u8; 600];
        let blob = Blob { stream: 0, flags: 0, seq: 0, payload: &payload };
        audio_push(&mut wire, &blob);
        audio_push(&mut wire, &blob);

        assert!(!audio_fits(&wire, &blob));
    }
}
