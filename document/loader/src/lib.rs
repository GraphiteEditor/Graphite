//! Opens Graphite documents through the format steps and node migrations this crate links, so every consumer reads documents the way the current build writes them.

use document_container::AnyContainer;
use document_container::archive::ArchiveFormat;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{NetworkMetadataEntry, NodeMetadataEntry};
use graph_craft::document::NodeNetwork;
use std::error::Error;
use std::path::{Path, PathBuf};

// Referenced so the linker keeps their self-registering migrations
use migration_compiled as _;
use migration_formats as _;
use migration_nodes as _;

/// A document's runtime network, with the file it was opened from.
pub struct LoadedDocument {
	pub network: NodeNetwork,
	pub origin: DocumentOrigin,
}

pub enum DocumentOrigin {
	/// A `.gdd`, with the metadata of the nodes and networks in its registry.
	Gdd {
		gdd: Box<GddV1>,
		nodes: Vec<NodeMetadataEntry>,
		networks: Vec<NetworkMetadataEntry>,
	},
	/// The JSON text of a legacy document.
	Legacy(String),
}

impl LoadedDocument {
	pub fn gdd(&self) -> Option<&GddV1> {
		match &self.origin {
			DocumentOrigin::Gdd { gdd, .. } => Some(gdd),
			DocumentOrigin::Legacy(_) => None,
		}
	}
}

/// Open a `.gdd` from its archive bytes into memory, running the migrations this crate links.
pub async fn open_gdd(archive: &[u8]) -> Result<GddV1, Box<dyn Error>> {
	let container = AnyContainer::Memory(MemoryBackend::new());
	let host = migration_compiled::CompilerHost::default();
	let gdd = GddV1::open_from_archive_with(archive, container, GddV1Layout, None, Some(&host))
		.await
		.map_err(|error| format!("Failed to open document: {error}"))?;
	Ok(gdd)
}

/// Open `path` as a `.gdd` if it is named `.gdd` or starts with archive magic bytes, otherwise as a legacy JSON document.
pub async fn open_document(path: &Path) -> Result<LoadedDocument, Box<dyn Error>> {
	let bytes = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;

	// A file named `.gdd` stays on the archive path, so a damaged one reports the archive error
	let named_gdd = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("gdd"));
	if !named_gdd && ArchiveFormat::detect(&bytes).is_none() {
		let document_string = String::from_utf8(bytes).map_err(|error| format!("Document {} is neither a .gdd archive nor text: {error}", path.display()))?;
		let network = legacy_network(&document_string).map_err(|error| format!("Failed to parse legacy document {}: {error}", path.display()))?;
		return Ok(LoadedDocument {
			network,
			origin: DocumentOrigin::Legacy(document_string),
		});
	}

	let gdd = open_gdd(&bytes).await?;
	let declarations = gdd.declarations(&gdd).await;
	let (network, nodes, networks) = gdd.registry().to_runtime_with_full_metadata(&declarations, gdd.blobs())?;
	Ok(LoadedDocument {
		network,
		origin: DocumentOrigin::Gdd { gdd: Box::new(gdd), nodes, networks },
	})
}

fn legacy_network(document_string: &str) -> Result<NodeNetwork, serde_json::Error> {
	let document: serde_json::Value = serde_json::from_str(document_string)?;
	let network = document["network_interface"]["network"].to_string();
	serde_json::from_str(&network)
}

/// The documents in the repository's `demo-artwork` folder, by file stem.
pub const DEMO_ARTWORK: [&str; 7] = [
	"changing-seasons",
	"painted-dreams",
	"red-dress",
	"valley-of-spires",
	"isometric-fountain",
	"procedural-string-lights",
	"parametric-dunescape",
];

pub fn demo_artwork_path(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-artwork").join(format!("{name}.graphite"))
}

/// The network of a demo artwork, opened with the migrations this crate links.
pub fn load_demo_artwork(name: &str) -> NodeNetwork {
	let path = demo_artwork_path(name);
	match futures::executor::block_on(open_document(&path)) {
		Ok(document) => document.network,
		Err(error) => panic!("Failed to open demo artwork {}: {error}", path.display()),
	}
}
