//! Bridge between [`crate::Codec`] and [`document_container::AnyContainer`]. Each payload's codec
//! is known up front (the manifest is always JSON; every other payload's codec is recorded in the
//! manifest), so reads and writes address a fixed `{basename}.{ext}` path without probing.

use document_container::{AnyContainer, AsyncContainer};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Codec, CodecError};

/// Compose a container path from `basename` and `codec.extension()`.
pub fn path_for(basename: &str, codec: Codec) -> String {
	format!("{basename}.{}", codec.extension())
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
	#[error("file not found for basename {basename:?} with codec {codec:?}")]
	NotFound { basename: String, codec: Codec },
	#[error("container error: {0}")]
	Container(#[from] document_container::ContainerError),
	#[error("codec error: {0}")]
	Codec(#[from] CodecError),
}

/// Read `{basename}.{ext}` and decode the single value it contains.
pub async fn read_single<T: DeserializeOwned>(container: &AnyContainer, basename: &str, codec: Codec) -> Result<T, ReadError> {
	let bytes = read_bytes(container, basename, codec).await?;
	Ok(codec.read_single::<T>(bytes.as_slice())?)
}

/// Same as [`read_single`] but yields every value when `codec` is a stream codec.
pub async fn iter<T: DeserializeOwned>(container: &AnyContainer, basename: &str, codec: Codec) -> Result<Vec<T>, ReadError> {
	let bytes = read_bytes(container, basename, codec).await?;
	Ok(codec.iter::<T>(bytes.as_slice()).collect::<Result<Vec<_>, _>>()?)
}

/// Whether `{basename}.{ext}` exists for the given codec.
pub async fn exists(container: &AnyContainer, basename: &str, codec: Codec) -> bool {
	container.exists(&path_for(basename, codec)).await
}

async fn read_bytes(container: &AnyContainer, basename: &str, codec: Codec) -> Result<document_container::ByteHolder, ReadError> {
	let path = path_for(basename, codec);
	if !container.exists(&path).await {
		return Err(ReadError::NotFound {
			basename: basename.to_string(),
			codec,
		});
	}
	Ok(container.read(&path).await?)
}

/// Encode `value` with `codec` and write to `{basename}.{ext}`. Synchronous: the write goes through
/// the container's sync write surface (durable on folder/memory, enqueued on OPFS).
pub fn write_single<T: Serialize>(container: &AnyContainer, basename: &str, codec: Codec, value: &T) -> Result<(), crate::Error> {
	let bytes = codec.write_single(value)?;
	container.write_non_blocking(&path_for(basename, codec), &bytes)?;
	Ok(())
}
