//! What the History panel shows, as the frontend receives it.

/// The panel's content: a line as interactions, newest first. The head's line with the undone ones above it,
/// or a branch's line while one is followed.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryPanelState {
	pub rows: Vec<HistoryRow>,
	/// Work under way in the room that has not entered history yet, one row per person.
	pub progress: Vec<HistoryProgressRow>,
	/// Whether older interactions exist beyond the rows sent.
	pub more: bool,
	/// The tip of the branch being followed instead of the head's line, if any.
	pub following: Option<String>,
	/// Whether the document is in a session, where moving the head moves it for everyone.
	pub session: bool,
}

/// One interaction: a run of deltas that undo takes back together.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryRow {
	/// The rev closing the interaction, in decimal.
	pub id: String,
	pub label: String,
	pub author: String,
	/// The author has no name on record, so `author` is a stand-in.
	pub anonymous: bool,
	/// The author is the person at this editor.
	pub mine: bool,
	pub color: String,
	/// When the interaction entered history, in milliseconds since the Unix epoch, if its retirer recorded it.
	pub time: Option<f64>,
	pub deltas: usize,
	/// Branches leaving this interaction that are not on the line shown, each by its tip.
	pub branches: Vec<HistoryBranch>,
	/// Above the head: taken back, and redo would bring it back.
	pub undone: bool,
	/// On a followed branch and not on the head's line: no head reaches it.
	pub abandoned: bool,
	pub head: bool,
	pub expanded: bool,
	/// The interaction's deltas, newest first, when expanded.
	pub details: Vec<HistoryDeltaRow>,
}

/// A branch leaving an interaction, named by its tip and what its newest interaction did.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryBranch {
	pub id: String,
	pub label: String,
}

/// One delta of an expanded interaction.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryDeltaRow {
	pub id: String,
	pub label: String,
	pub author: String,
	pub color: String,
	pub time: Option<f64>,
}

/// A person with ops in the room's hot log: their open or not yet retired work.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryProgressRow {
	/// What the work changes so far, in the words of the rows.
	pub label: String,
	pub author: String,
	pub anonymous: bool,
	pub mine: bool,
	pub color: String,
	pub ops: usize,
	/// Still being made, rather than closed and waiting to enter history.
	pub open: bool,
}
