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
	/// A packet arrived for a document's room: poll it now rather than at the next frame. `generation`
	/// names the connection it was armed for, so a wake from an earlier connection is ignored.
	Wake { document_id: DocumentId, generation: u32 },
	/// A document's working copy mounted; attaches it to a pending join.
	StorageMounted { document_id: DocumentId },
	/// The transport loop of a document's session ended.
	Disconnected { document_id: DocumentId },
	/// A declaration a remote change names was already on hand and was read from the byte store to be
	/// decoded; `None` when the store turned out not to hold it after all.
	DeclarationLoaded { document_id: DocumentId, hash: ResourceHash, bytes: Option<Vec<u8>> },
	/// A resource a peer asked for was read from the byte store.
	ResourceLoaded {
		document_id: DocumentId,
		to: TransportPeerId,
		hash: ResourceHash,
		bytes: Vec<u8>,
	},
}
