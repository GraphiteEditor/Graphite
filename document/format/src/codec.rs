//! Codec for a stream of values. Single-value writes are just streams of length one.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
	/// A single JSON document. `append` to a non-empty buffer errors.
	Json,
	/// Newline-delimited compact JSON, one value per line.
	JsonLines,
	/// A single MessagePack blob. `append` to a non-empty buffer errors.
	MessagePack,
	/// Length-prefixed MessagePack frames: `[u32 big-endian length][MessagePack bytes]` per value.
	MessagePackFrames,
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
	#[error("MessagePack encode error: {0}")]
	MessagePackEncode(#[from] rmp_serde::encode::Error),
	#[error("MessagePack decode error: {0}")]
	MessagePackDecode(#[from] rmp_serde::decode::Error),
	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),
	#[error("frame length {0} exceeds u32")]
	FrameTooLarge(usize),
	#[error("frame length prefix truncated: need 4 bytes, have {0}")]
	TruncatedLengthPrefix(usize),
	#[error("declared frame length {declared} exceeds remaining buffer ({remaining} bytes)")]
	TruncatedFrame { declared: usize, remaining: usize },
	#[error("single-value codec cannot append to a non-empty buffer")]
	SingleValueAlreadyWritten,
	#[error("expected at least one value, got none")]
	Empty,
	#[error("expected exactly one value, got more")]
	ExpectedSingle,
}

impl Codec {
	pub fn extension(self) -> &'static str {
		match self {
			Codec::Json => "json",
			Codec::JsonLines => "jsonl",
			Codec::MessagePack => "bin",
			Codec::MessagePackFrames => "frames",
		}
	}

	/// Append one value to `output` in this codec's framing.
	/// Single-value codecs error if `output` is non-empty.
	pub fn append<T: Serialize>(self, output: &mut Vec<u8>, value: &T) -> Result<(), CodecError> {
		match self {
			Codec::Json => {
				if !output.is_empty() {
					return Err(CodecError::SingleValueAlreadyWritten);
				}
				serde_json::to_writer_pretty(output, value)?;
				Ok(())
			}
			Codec::JsonLines => {
				serde_json::to_writer(&mut *output, value)?;
				output.push(b'\n');
				Ok(())
			}
			Codec::MessagePack => {
				if !output.is_empty() {
					return Err(CodecError::SingleValueAlreadyWritten);
				}
				rmp_serde::encode::write(output, value)?;
				Ok(())
			}
			Codec::MessagePackFrames => {
				let payload = rmp_serde::to_vec(value)?;
				let length = u32::try_from(payload.len()).map_err(|_| CodecError::FrameTooLarge(payload.len()))?;
				output.extend_from_slice(&length.to_be_bytes());
				output.extend_from_slice(&payload);
				Ok(())
			}
		}
	}

	/// Iterate values from `bytes`. Single-value codecs yield exactly one item;
	/// stream codecs yield however many were written.
	pub fn iter<'a, T: DeserializeOwned + 'a>(self, bytes: &'a [u8]) -> Box<dyn Iterator<Item = Result<T, CodecError>> + 'a> {
		match self {
			Codec::Json => {
				let single = serde_json::from_slice::<T>(bytes).map_err(CodecError::from);
				Box::new(std::iter::once(single))
			}
			Codec::JsonLines => Box::new(JsonLineIter {
				remaining: bytes,
				_marker: std::marker::PhantomData,
			}),
			Codec::MessagePack => {
				let single = rmp_serde::from_slice::<T>(bytes).map_err(CodecError::from);
				Box::new(std::iter::once(single))
			}
			Codec::MessagePackFrames => Box::new(MessagePackFrameIter {
				remaining: bytes,
				_marker: std::marker::PhantomData,
			}),
		}
	}

	/// Serialize a single value into a fresh buffer.
	pub fn write_single<T: Serialize>(self, value: &T) -> Result<Vec<u8>, CodecError> {
		let mut output = Vec::new();
		self.append(&mut output, value)?;
		Ok(output)
	}

	/// Deserialize the single value in `bytes`. Errors if zero or more than one value is present.
	pub fn read_single<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T, CodecError> {
		let mut iter = self.iter::<T>(bytes);
		let first = iter.next().ok_or(CodecError::Empty)??;
		match iter.next() {
			// A trailing decode error is more informative than `ExpectedSingle`, so surface it.
			Some(Err(error)) => return Err(error),
			Some(Ok(_)) => return Err(CodecError::ExpectedSingle),
			None => {}
		}
		Ok(first)
	}
}

struct JsonLineIter<'a, T> {
	remaining: &'a [u8],
	_marker: std::marker::PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> Iterator for JsonLineIter<'_, T> {
	type Item = Result<T, CodecError>;

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			if self.remaining.is_empty() {
				return None;
			}

			let (line, tail) = match self.remaining.iter().position(|&byte| byte == b'\n') {
				Some(index) => (&self.remaining[..index], &self.remaining[index + 1..]),
				None => (self.remaining, &[][..]),
			};
			self.remaining = tail;

			let trimmed = trim_ascii(line);
			if trimmed.is_empty() {
				continue;
			}

			return Some(serde_json::from_slice(trimmed).map_err(CodecError::from));
		}
	}
}

struct MessagePackFrameIter<'a, T> {
	remaining: &'a [u8],
	_marker: std::marker::PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> Iterator for MessagePackFrameIter<'_, T> {
	type Item = Result<T, CodecError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.remaining.is_empty() {
			return None;
		}

		let buffer = std::mem::take(&mut self.remaining);

		let Some((length_bytes, tail)) = buffer.split_first_chunk::<4>() else {
			return Some(Err(CodecError::TruncatedLengthPrefix(buffer.len())));
		};
		let length = u32::from_be_bytes(*length_bytes) as usize;

		if tail.len() < length {
			return Some(Err(CodecError::TruncatedFrame {
				declared: length,
				remaining: tail.len(),
			}));
		}

		let (frame, after) = tail.split_at(length);
		self.remaining = after;

		Some(rmp_serde::from_slice(frame).map_err(CodecError::from))
	}
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
	let start = bytes.iter().position(|byte| !byte.is_ascii_whitespace()).unwrap_or(bytes.len());
	let end = bytes.iter().rposition(|byte| !byte.is_ascii_whitespace()).map(|index| index + 1).unwrap_or(start);
	&bytes[start..end]
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
	struct Frame {
		id: u32,
		label: String,
	}

	fn frames() -> [Frame; 3] {
		[Frame { id: 1, label: "alpha".into() }, Frame { id: 2, label: "beta".into() }, Frame { id: 3, label: "gamma".into() }]
	}

	#[test]
	fn json_round_trip_single() {
		let frame = Frame { id: 7, label: "solo".into() };
		let bytes = Codec::Json.write_single(&frame).unwrap();
		let decoded: Frame = Codec::Json.read_single(&bytes).unwrap();
		assert_eq!(decoded, frame);
	}

	#[test]
	fn json_append_to_non_empty_errors() {
		let mut buffer = b"already here".to_vec();
		let result = Codec::Json.append(&mut buffer, &Frame { id: 1, label: "x".into() });
		assert!(matches!(result, Err(CodecError::SingleValueAlreadyWritten)), "got {result:?}");
	}

	#[test]
	fn message_pack_round_trip_single() {
		let frame = Frame { id: 99, label: "blob".into() };
		let bytes = Codec::MessagePack.write_single(&frame).unwrap();
		let decoded: Frame = Codec::MessagePack.read_single(&bytes).unwrap();
		assert_eq!(decoded, frame);
	}

	#[test]
	fn message_pack_append_to_non_empty_errors() {
		let mut buffer = vec![0xAB];
		let result = Codec::MessagePack.append(&mut buffer, &Frame { id: 1, label: "x".into() });
		assert!(matches!(result, Err(CodecError::SingleValueAlreadyWritten)), "got {result:?}");
	}

	/// A type-erased `serde_json::Value` round-trips through the binary codec: the property postcard
	/// could not satisfy (it raises `WontImplement` on self-describing values), which is why the
	/// resource/attribute deltas that carry `serde_json::Value` bodies need a self-describing codec.
	#[test]
	fn message_pack_round_trips_serde_json_value() {
		let value = serde_json::json!({ "kind": "embedded", "priority": 1.5, "tags": ["a", "b"] });
		let bytes = Codec::MessagePack.write_single(&value).unwrap();
		let decoded: serde_json::Value = Codec::MessagePack.read_single(&bytes).unwrap();
		assert_eq!(decoded, value);
	}

	#[test]
	fn json_lines_round_trip_and_skip_blanks() {
		let frames = [Frame { id: 1, label: "alpha".into() }, Frame { id: 2, label: "beta".into() }];

		let mut buffer = Vec::new();
		Codec::JsonLines.append(&mut buffer, &frames[0]).unwrap();
		buffer.extend_from_slice(b"   \n\n");
		Codec::JsonLines.append(&mut buffer, &frames[1]).unwrap();

		let decoded: Vec<Frame> = Codec::JsonLines.iter(&buffer).collect::<Result<_, _>>().unwrap();
		assert_eq!(decoded, frames);
	}

	#[test]
	fn message_pack_frames_round_trip() {
		let frames = frames();
		let mut buffer = Vec::new();
		for frame in &frames {
			Codec::MessagePackFrames.append(&mut buffer, frame).unwrap();
		}
		let decoded: Vec<Frame> = Codec::MessagePackFrames.iter(&buffer).collect::<Result<_, _>>().unwrap();
		assert_eq!(decoded, frames);
	}

	/// A crash mid-append leaves a torn final frame. The length prefix lets us detect that
	/// deterministically (declared length exceeds the bytes that actually made it to disk) rather
	/// than decoding a partial value into a plausible-but-wrong one.
	#[test]
	fn message_pack_frames_detect_truncation() {
		let mut buffer = Vec::new();
		Codec::MessagePackFrames.append(&mut buffer, &Frame { id: 7, label: "ok".into() }).unwrap();
		buffer.truncate(buffer.len() - 1);
		let last = Codec::MessagePackFrames.iter::<Frame>(&buffer).last().unwrap();
		assert!(matches!(last, Err(CodecError::TruncatedFrame { .. })), "got {last:?}");
	}

	/// A buffer whose first record's length prefix itself is incomplete (fewer than 4 bytes) is
	/// reported as a truncated prefix rather than mis-read as a zero-length frame.
	#[test]
	fn message_pack_frames_detect_truncated_length_prefix() {
		let buffer = vec![0x00, 0x00];
		let last = Codec::MessagePackFrames.iter::<Frame>(&buffer).last().unwrap();
		assert!(matches!(last, Err(CodecError::TruncatedLengthPrefix(2))), "got {last:?}");
	}

	#[test]
	fn write_single_then_read_with_iter_yields_one() {
		let frame = Frame { id: 5, label: "one".into() };
		for codec in [Codec::Json, Codec::JsonLines, Codec::MessagePack, Codec::MessagePackFrames] {
			let bytes = codec.write_single(&frame).unwrap();
			let collected: Vec<Frame> = codec.iter(&bytes).collect::<Result<_, _>>().unwrap();
			assert_eq!(collected, vec![Frame { id: 5, label: "one".into() }], "codec {codec:?}");
		}
	}

	#[test]
	fn read_single_rejects_multi_value_stream() {
		let mut buffer = Vec::new();
		Codec::JsonLines.append(&mut buffer, &Frame { id: 1, label: "a".into() }).unwrap();
		Codec::JsonLines.append(&mut buffer, &Frame { id: 2, label: "b".into() }).unwrap();
		let result: Result<Frame, _> = Codec::JsonLines.read_single(&buffer);
		assert!(matches!(result, Err(CodecError::ExpectedSingle)), "got {result:?}");
	}

	#[test]
	fn extensions_are_distinct() {
		let exts = [
			Codec::Json.extension(),
			Codec::JsonLines.extension(),
			Codec::MessagePack.extension(),
			Codec::MessagePackFrames.extension(),
		];
		let unique: std::collections::HashSet<_> = exts.iter().collect();
		assert_eq!(unique.len(), exts.len(), "extensions collide: {exts:?}");
	}
}
