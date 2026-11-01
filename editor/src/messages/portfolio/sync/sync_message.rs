use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, Sync)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SyncMessage {
	/// Retire what is due, once per frame.
	Poll,
	/// A document's working copy mounted.
	StorageMounted,
}
