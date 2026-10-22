use std::io::{Read, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ProtocolLimits;

pub fn write_framed<T: Serialize>(
    writer: &mut impl Write,
    value: &T,
    limits: ProtocolLimits,
) -> Result<(), FramingError> {
    let bytes = serde_json::to_vec(value).map_err(FramingError::Encode)?;
    if bytes.len() as u64 > limits.max_payload_bytes {
        return Err(FramingError::PayloadTooLarge {
            actual: bytes.len() as u64,
            maximum: limits.max_payload_bytes,
        });
    }
    let length = u32::try_from(bytes.len()).map_err(|_| FramingError::PayloadTooLarge {
        actual: bytes.len() as u64,
        maximum: u32::MAX as u64,
    })?;
    writer
        .write_all(&length.to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .and_then(|_| writer.flush())
        .map_err(FramingError::Io)
}

pub fn read_framed<T: DeserializeOwned>(
    reader: &mut impl Read,
    limits: ProtocolLimits,
) -> Result<Option<T>, FramingError> {
    let mut header = [0_u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(FramingError::Io(error)),
    }
    let length = u32::from_le_bytes(header) as u64;
    if length > limits.max_payload_bytes {
        return Err(FramingError::PayloadTooLarge {
            actual: length,
            maximum: limits.max_payload_bytes,
        });
    }
    let mut bytes = vec![0; length as usize];
    reader.read_exact(&mut bytes).map_err(FramingError::Io)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(FramingError::Decode)
}

#[derive(Debug, thiserror::Error)]
pub enum FramingError {
    #[error("failed to encode plugin message: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("failed to decode plugin message: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("plugin IPC payload has {actual} bytes; maximum is {maximum}")]
    PayloadTooLarge { actual: u64, maximum: u64 },
    #[error("plugin IPC failed: {0}")]
    Io(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_round_trips_and_rejects_oversize_headers() {
        let limits = ProtocolLimits {
            max_payload_bytes: 32,
            max_queued_messages: 1,
        };
        let mut bytes = Vec::new();
        write_framed(&mut bytes, &"preview", limits).unwrap();
        assert_eq!(
            read_framed::<String>(&mut bytes.as_slice(), limits)
                .unwrap()
                .as_deref(),
            Some("preview")
        );

        let oversize_header = 33_u32.to_le_bytes();
        let mut oversize = oversize_header.as_slice();
        assert!(matches!(
            read_framed::<String>(&mut oversize, limits),
            Err(FramingError::PayloadTooLarge { .. })
        ));
    }
}
