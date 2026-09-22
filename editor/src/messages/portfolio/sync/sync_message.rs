use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceHash;
use peer_transport::TransportPeerId;

#[impl_message(Message, PortfolioMessage, Sync)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SyncMessage {
	/// Host the active document and hand the frontend a link for others to join.
	Share,
	/// Open a new document that follows the session behind `token`.
	Join { token: String },
	/// Leave the active document's session.
	Leave,
	/// Per frame: apply what peers sent and answer their requests.
	Poll,
	/// A document's working copy mounted; attaches it to a pending join.
	StorageMounted { document_id: DocumentId },
	/// The transport loop of a document's session ended.
	Disconnected { document_id: DocumentId },
	/// A resource a peer asked for was read from the byte store.
	ResourceLoaded {
		document_id: DocumentId,
		to: TransportPeerId,
		hash: ResourceHash,
		bytes: Vec<u8>,
	},
	/// A resource received from a peer was copied into the byte store.
	ResourceStored { document_id: DocumentId, hash: ResourceHash, bytes: Vec<u8> },
}
