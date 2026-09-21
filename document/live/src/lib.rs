//! A document that lives in a collaborative session rather than on disk. Hosts a session or follows
//! one, and hands out the current runtime `NodeNetwork` on demand. Owns no executor, window, or
//! async runtime: the caller spawns the transport driver and decides how to render.

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout, ResourceProxy};
use document_graph_storage::{PeerId, UserId};
use graph_craft::document::NodeNetwork;
use peer_transport::{Room, SyncTarget};

pub use document_format::Error as FormatError;
pub use document_graph_storage::to_runtime::ConversionError;
pub use peer_transport::{Event, MessageLoopFuture, Role, SessionToken};

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
	#[error(transparent)]
	Format(#[from] FormatError),
	#[error(transparent)]
	Conversion(#[from] ConversionError),
}

pub struct LiveDocument {
	gdd: GddV1,
	token: SessionToken,
}

impl LiveDocument {
	/// Create an empty document and host a session under `token`. `spawn` must drive the returned
	/// future for as long as the session lives.
	pub async fn host(signaling_server: &str, token: SessionToken, peer: PeerId, user: UserId, spawn: impl FnOnce(MessageLoopFuture)) -> Result<Self, LiveError> {
		let gdd = Self::empty_document(peer).await?;
		Ok(Self::host_document(gdd, signaling_server, token, user, spawn))
	}

	/// Host a session on an already opened document.
	pub fn host_document(mut gdd: GddV1, signaling_server: &str, token: SessionToken, user: UserId, spawn: impl FnOnce(MessageLoopFuture)) -> Self {
		let (room, driver) = Room::connect(&token.signaling_url(signaling_server));
		spawn(driver);
		gdd.share(room, user);
		Self { gdd, token }
	}

	/// Join the session under `token`; the document fills in once the host's sync arrives.
	pub async fn join(signaling_server: &str, token: SessionToken, peer: PeerId, user: UserId, spawn: impl FnOnce(MessageLoopFuture)) -> Result<Self, LiveError> {
		let mut gdd = Self::empty_document(peer).await?;
		let (room, driver) = Room::connect(&token.signaling_url(signaling_server));
		spawn(driver);
		gdd.join(room, user);
		Ok(Self { gdd, token })
	}

	async fn empty_document(peer: PeerId) -> Result<GddV1, FormatError> {
		let version = env!("CARGO_PKG_VERSION").to_string();
		GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, peer, peer.0, version.clone(), version).await
	}

	pub fn token(&self) -> SessionToken {
		self.token
	}

	pub fn role(&self) -> Option<Role> {
		self.gdd.role()
	}

	pub fn is_synced(&self) -> bool {
		self.gdd.is_synced()
	}

	/// Synced and holding every resource the document references, so `network` can be built and run.
	pub fn is_ready(&self) -> bool {
		self.is_synced() && SyncTarget::missing_resources(&self.gdd).is_empty()
	}

	/// Apply whatever peers sent since the last call and answer their resource requests. Call it regularly.
	pub async fn poll(&mut self) -> Vec<Event> {
		let events = self.gdd.poll_peers();
		for event in &events {
			let Event::ResourceRequested { from, hash } = event else { continue };
			match self.gdd.read_resource(hash).await {
				Ok(bytes) => {
					if let Err(error) = self.gdd.send_resource(*from, *hash, bytes.as_slice().to_vec()) {
						log::warn!("Failed to send resource {hash}: {error}");
					}
				}
				Err(error) => log::warn!("Peer asked for resource {hash} which is not in this document: {error}"),
			}
		}
		events
	}

	pub fn document(&self) -> &GddV1 {
		&self.gdd
	}

	pub fn document_mut(&mut self) -> &mut GddV1 {
		&mut self.gdd
	}

	/// The current runtime graph, rebuilt from the registry. Async because proto-node declarations are resources.
	pub async fn network(&self) -> Result<NodeNetwork, LiveError> {
		let declarations = self.gdd.declarations(&self.gdd).await;
		let (network, _metadata) = self.gdd.registry().to_runtime_with_metadata(&declarations)?;
		Ok(network)
	}

	/// Byte store for resources received from peers, for an executor to load from.
	pub fn resource_proxy(&self) -> ResourceProxy<GddV1Layout> {
		self.gdd.resource_proxy()
	}

	pub fn leave(&mut self) {
		self.gdd.leave();
	}
}
