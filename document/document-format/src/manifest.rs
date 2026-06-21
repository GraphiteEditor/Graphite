//! Bootstrap file for a `.gdd` document. Always JSON regardless of payload codec choice.

use serde::{Deserialize, Serialize};

use crate::Codec;
use crate::{DEFAULT_HISTORY_CODEC, DEFAULT_HOT_LOG_CODEC, DEFAULT_REGISTRY_CODEC, DEFAULT_SESSION_CODEC};

/// Magic string carried in [`Manifest::format`] to identify a `.gdd` document.
pub const FORMAT_MAGIC: &str = "gdd";

/// Maximum manifest version this build can open. Bumped when manifest layout changes
/// in a way that older builds can't safely read.
pub const SUPPORTED_FORMAT_VERSION: u32 = 1;

/// The on-disk codec for each working-copy payload, recorded so reads/writes never have to probe
/// the filesystem to discover it. The manifest itself is excluded: it is always JSON, since it must
/// be parsed before any other codec is known.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PayloadCodecs {
	pub registry: Codec,
	pub history: Codec,
	pub hot_log: Codec,
	pub session: Codec,
}

impl Default for PayloadCodecs {
	fn default() -> Self {
		Self {
			registry: DEFAULT_REGISTRY_CODEC,
			history: DEFAULT_HISTORY_CODEC,
			hot_log: DEFAULT_HOT_LOG_CODEC,
			session: DEFAULT_SESSION_CODEC,
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
	pub format: String,

	pub format_version: u32,
	pub editor_version: String,
	pub stdlib_version: String,

	pub document_id: u64,
	/// Codec used for each non-manifest payload on disk. Authoritative — never inferred from which
	/// file extension is present.
	#[serde(default)]
	pub codecs: PayloadCodecs,
}

impl Manifest {
	pub fn new(document_id: u64, editor_version: String, stdlib_version: String) -> Self {
		Self {
			format: FORMAT_MAGIC.to_string(),
			format_version: SUPPORTED_FORMAT_VERSION,
			document_id,
			editor_version,
			stdlib_version,
			codecs: PayloadCodecs::default(),
		}
	}
}
