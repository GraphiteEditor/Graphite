use crate::messages::prelude::*;

#[impl_message(Message, PortfolioMessage, History)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HistoryMessage {
	/// Send the History panel's content for the active document, whether or not anything changed.
	Refresh,
	/// Once per frame while the panel is open: send the content again if the history moved since the last send.
	Tick,
	/// Show or hide the deltas of one interaction, named by the rev that closes it.
	Expand { id: String, expanded: bool },
	/// Show older interactions than the ones on hand.
	LoadMore,
}
