use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceHash;
use peer_transport::TransportPeerId;

#[impl_message(Message, PortfolioMessage, Sync)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SyncMessage {
	/// Host the active document and give the frontend a link.
	Share,
	/// Open a new document that follows the session behind `token`.
	Join { token: String },
	/// Disconnect the active document. Editing continues alone.
	Leave,
	/// Host the active document as a new session with its own link. The others stay in the old room.
	Fork,
	/// Send the Collaboration panel's layout.
	RefreshPanel,
	/// Apply what peers sent, once per frame.
	Poll,
	/// A packet arrived, so poll now. A stale `generation` is ignored.
	Wake { document_id: DocumentId, generation: u32 },
	/// A document's working copy mounted.
	StorageMounted { document_id: DocumentId },
	/// A transport loop ended. A stale `generation` is ignored.
	Disconnected { document_id: DocumentId, generation: u32 },
	/// `None` when the byte store lacks the declaration.
	DeclarationLoaded { document_id: DocumentId, hash: ResourceHash, bytes: Option<Vec<u8>> },
	/// A resource a peer asked for was read from the byte store.
	ResourceLoaded {
		document_id: DocumentId,
		to: TransportPeerId,
		hash: ResourceHash,
		bytes: Vec<u8>,
	},
}
