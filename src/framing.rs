use crate::protocol::{BridgeError, Envelope, MAX_FRAME};
use std::io::{Read, Write};

pub fn read_native<R: Read>(reader: &mut R) -> Result<Option<Envelope>, BridgeError> {
    let mut header = [0; 4];
    match reader.read(&mut header[..1]) {
        Ok(0) => return Ok(None),
        Ok(_) => (),
        Err(error) => return Err(io_error(error, BridgeError::Disconnected)),
    }
    reader
        .read_exact(&mut header[1..])
        .map_err(|error| io_error(error, BridgeError::InvalidRequest))?;
    let length = u32::from_ne_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(BridgeError::TooLarge);
    }
    let mut bytes = vec![0; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| io_error(error, BridgeError::InvalidRequest))?;
    let frame: Envelope =
        serde_json::from_slice(&bytes).map_err(|_| BridgeError::InvalidRequest)?;
    frame.validate()?;
    Ok(Some(frame))
}
pub fn write_native<W: Write>(writer: &mut W, frame: &Envelope) -> Result<(), BridgeError> {
    frame.validate()?;
    let bytes = serde_json::to_vec(frame).map_err(|_| BridgeError::InvalidRequest)?;
    if bytes.len() > MAX_FRAME {
        return Err(BridgeError::TooLarge);
    }
    writer
        .write_all(&(bytes.len() as u32).to_ne_bytes())
        .and_then(|()| writer.write_all(&bytes))
        .and_then(|()| writer.flush())
        .map_err(|error| io_error(error, BridgeError::Disconnected))
}

fn io_error(error: std::io::Error, fallback: BridgeError) -> BridgeError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        BridgeError::Timeout
    } else {
        fallback
    }
}
