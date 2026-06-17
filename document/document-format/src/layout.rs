//! Path layout for a `.gdd` working copy.
//!
//! Layout owns basenames only — the codec choice for each payload is a runtime parameter at the
//! read/write call site. Working-copy creation, exports, and migrations may all hit the same
//! basename with different codecs.

use graphene_resource::ResourceHash;

pub trait Layout {
	fn manifest_basename(&self) -> &str;
	fn session_basename(&self) -> &str;
	fn registry_basename(&self) -> &str;
	fn history_basename(&self) -> &str;
	fn hot_log_basename(&self) -> &str;
	fn resources_dir(&self) -> &str;
	fn resource_path(&self, hash: &ResourceHash) -> String;
	/// The embedded legacy `.graphite` document, stored verbatim during the dual-write soak so the
	/// new format can be validated against (and recovered from) the old one. Dropped once `.gdd`
	/// becomes the sole source of truth.
	fn legacy_basename(&self) -> &str;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct GddV1Layout;

impl Layout for GddV1Layout {
	fn manifest_basename(&self) -> &str {
		"manifest"
	}
	fn session_basename(&self) -> &str {
		"session"
	}
	fn registry_basename(&self) -> &str {
		"registry"
	}
	fn history_basename(&self) -> &str {
		"history"
	}
	fn hot_log_basename(&self) -> &str {
		"hot-log"
	}
	fn resources_dir(&self) -> &str {
		"resources"
	}
	fn resource_path(&self, hash: &ResourceHash) -> String {
		format!("{}/{hash}", self.resources_dir())
	}
	fn legacy_basename(&self) -> &str {
		"legacy.graphite"
	}
}
