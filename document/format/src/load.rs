//! Reads a working copy's payloads as records before decoding them, so format migrations can run in between.

use document_container::{AnyContainer, AsyncContainer};
use document_graph_storage::{Delta, HotOp, Registry};

use crate::error::Error;
use crate::layout::Layout;
use crate::manifest::PayloadCodecs;
use crate::{Codec, SessionState, io};

/// Each payload's records in their stored encoding. `None` is a payload the working copy doesn't have.
pub(crate) struct StoredRecords {
	pub registry: Option<Vec<u8>>,
	pub history: Option<Vec<Vec<u8>>>,
	pub hot_log: Vec<Vec<u8>>,
	pub session: Option<Vec<u8>>,
}

/// The payloads in the live types.
pub(crate) struct DecodedPayloads {
	pub registry: Option<Registry>,
	pub history: Option<Vec<Delta>>,
	pub hot_log: Vec<HotOp>,
	pub session: SessionState,
}

impl StoredRecords {
	pub async fn read<L: Layout>(working: &AnyContainer, layout: &L, codecs: PayloadCodecs) -> Result<Self, Error> {
		Ok(Self {
			registry: read_records(working, layout.registry_basename(), codecs.registry).await?.map(single_record).transpose()?,
			history: read_records(working, layout.history_basename(), codecs.history).await?,
			hot_log: read_records(working, layout.hot_log_basename(), codecs.hot_log).await?.unwrap_or_default(),
			session: read_records(working, layout.session_basename(), codecs.session).await?.map(single_record).transpose()?,
		})
	}

	pub fn decode(self, codecs: PayloadCodecs) -> Result<DecodedPayloads, Error> {
		Ok(DecodedPayloads {
			registry: self.registry.map(|record| codecs.registry.decode_record(&record)).transpose()?,
			history: self.history.map(|records| decode_all(records, codecs.history)).transpose()?,
			hot_log: decode_all(self.hot_log, codecs.hot_log)?,
			session: self.session.map(|record| codecs.session.decode_record(&record)).transpose()?.unwrap_or_default(),
		})
	}
}

/// The one record of a single-value payload. An existing file without exactly one is corrupt, not absent.
fn single_record(records: Vec<Vec<u8>>) -> Result<Vec<u8>, crate::CodecError> {
	let mut records = records.into_iter();
	let first = records.next().ok_or(crate::CodecError::Empty)?;

	match records.next() {
		Some(_) => Err(crate::CodecError::ExpectedSingle),
		None => Ok(first),
	}
}

fn decode_all<T: serde::de::DeserializeOwned>(records: Vec<Vec<u8>>, codec: Codec) -> Result<Vec<T>, crate::CodecError> {
	records.iter().map(|record| codec.decode_record(record)).collect()
}

async fn read_records(working: &AnyContainer, basename: &str, codec: Codec) -> Result<Option<Vec<Vec<u8>>>, Error> {
	let path = io::path_for(basename, codec);
	if !working.exists(&path).await {
		return Ok(None);
	}

	let bytes = working.read(&path).await?;
	let records = codec.records(bytes.as_slice())?.into_iter().map(<[u8]>::to_vec).collect();
	Ok(Some(records))
}
