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
	/// Move the head to the interaction: back along the line, or forward again over undone ones.
	GoBack { id: String },
	/// Drop one interaction out of the line, the later ones staying.
	RemoveStep { id: String },
	/// Bring an undone or abandoned interaction back.
	BringBack { id: String },
	/// Show the line of the branch whose tip is `id`, or the head's line again with `None`.
	Follow { id: Option<String> },
	/// Give the interaction a name of its own, or take it away with an empty one.
	Rename { id: String, label: String },
	/// Put a tag on the interaction, or take it off.
	Tag { id: String, tag: String, on: bool },
}
