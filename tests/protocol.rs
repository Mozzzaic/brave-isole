use brave_isole::{
    framing::{read_native, write_native},
    protocol::{Envelope, MAX_FRAME, Payload, Reply},
};
use std::io::Cursor;

fn reply(text: &str) -> Envelope {
    Envelope {
        version: 1,
        expires_at_ms: None,
        epoch: "epoch".into(),
        id: "id".into(),
        payload: Payload::Reply {
            reply: Reply {
                ok: true,
                result: Some(serde_json::Value::String(text.into())),
                error: None,
            },
        },
    }
}
fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut framed = (bytes.len() as u32).to_ne_bytes().to_vec();
    framed.extend_from_slice(bytes);
    framed
}
#[test]
fn eof_between_frames_is_clean() {
    assert!(
        read_native(&mut Cursor::new([]))
            .expect("clean EOF")
            .is_none()
    );
}
#[test]
fn truncated_header_rejected() {
    assert!(read_native(&mut Cursor::new([1, 0])).is_err());
}
#[test]
fn truncated_body_rejected() {
    assert!(read_native(&mut Cursor::new([9, 0, 0, 0, 123])).is_err());
}
#[test]
fn unknown_fields_rejected() {
    let bytes = br#"{"version":1,"epoch":"e","id":"i","payload":{"type":"command","role":"codex","command":{"op":"list","shell":"evil"}}}"#;
    assert!(read_native(&mut Cursor::new(frame(bytes))).is_err());
}
#[test]
fn unknown_version_rejected() {
    let mut value = reply("ok");
    value.version = 2;
    assert!(
        read_native(&mut Cursor::new(frame(
            &serde_json::to_vec(&value).expect("JSON")
        )))
        .is_err()
    );
}
#[test]
fn native_exact_limit_round_trip() {
    let overhead = serde_json::to_vec(&reply("")).expect("JSON").len();
    let value = reply(&"a".repeat(MAX_FRAME - overhead));
    let mut output = vec![];
    write_native(&mut output, &value).expect("exact limit");
    assert_eq!(output.len(), 1_048_580);
    assert_eq!(
        read_native(&mut Cursor::new(output)).expect("frame"),
        Some(value)
    );
}
#[test]
fn native_limit_is_utf8_bytes() {
    let overhead = serde_json::to_vec(&reply("")).expect("JSON").len();
    let value = reply(&"a".repeat(MAX_FRAME - overhead + 1));
    assert!(write_native(&mut vec![], &value).is_err());
    let length = (1_048_577_u32).to_ne_bytes();
    assert!(read_native(&mut Cursor::new(length)).is_err());
}
#[test]
fn emoji_and_base64_budget() {
    let value = reply(&"😀".repeat(262_144));
    assert!(write_native(&mut vec![], &value).is_err());
    let value = reply(&"YWFh".repeat(262_144));
    assert!(write_native(&mut vec![], &value).is_err());
}
#[test]
fn io_timeout_is_reported_as_timeout() {
    struct Expired;
    impl std::io::Read for Expired {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::TimedOut))
        }
    }
    assert_eq!(
        read_native(&mut Expired),
        Err(brave_isole::protocol::BridgeError::Timeout)
    );
}
