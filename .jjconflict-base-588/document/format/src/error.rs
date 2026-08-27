//! Unified error type for the `document-format` crate.
//!
//! Every fallible [`crate::Gdd`] method returns [`Result<T>`]. Variants are grouped by failure
//! domain (container I/O, codec, CRDT, format validation, export)

use document_container::ContainerError;
#[cfg(feature = "conversion")]
use document_graph_storage::CommitError;
use document_graph_storage::CrdtError;
use graphene_resource::ResourceHash;

use crate::codec::CodecError;
use crate::io::ReadError;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Anything that can go wrong reading, mutating, or exporting a `.gdd` document. Per the format's
/// load-time policy, any unexpected condition is a hard error rather than a silent fallback.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	/// Working-copy container I/O failed (read, write, or path validation).
	#[error("container error: {0}")]
	Container(#[from] ContainerError),
	/// A typed payload could not be located or read from the container.
	#[error("read error: {0}")]
	Read(#[from] ReadError),
	/// A payload failed to encode or decode in its recorded codec.
	#[error("codec error: {0}")]
	Codec(#[from] CodecError),
	/// A CRDT operation was rejected while replaying a hot op or moving the undo/redo cursor.
	#[error("CRDT error: {0}")]
	Crdt(#[from] CrdtError),
	/// Staging a runtime snapshot into the session failed (conversion or CRDT apply).
	#[cfg(feature = "conversion")]
	#[error("commit error: {0}")]
	Commit(#[from] CommitError),
	/// The manifest's `format` field is not the `.gdd` magic, so this is not a `.gdd` document.
	#[error("not a .gdd document (manifest format = {found:?}, expected {expected:?})")]
	WrongFormat { found: String, expected: &'static str },
	/// The manifest declares a format version newer than this build can open.
	#[error("unsupported format version: found {found}, max supported {max_supported}")]
	UnsupportedVersion { found: u32, max_supported: u32 },
	/// The requested export options are incoherent (e.g. neither registry nor history included).
	#[error("invalid export options: {0}")]
	InvalidExportOptions(&'static str),
	/// An export marked a resource for embedding but its bytes were absent from the byte store.
	#[error("embedded resource {0} missing from the byte store")]
	MissingResource(ResourceHash),
}
