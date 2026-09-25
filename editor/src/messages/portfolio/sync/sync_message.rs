use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceHash;
use peer_transport::TransportPeerId;

#[impl_message(Message, PortfolioMessage, Sync)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SyncMessage {
	/// Host the active document in the room every copy of it shares, and hand the frontend a link for others.
	Share,
	/// Open a new document that follows the session behind `token`.
	Join { token: String },
	/// Disconnect the active document from its room; editing goes on alone until the next share.
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
}
