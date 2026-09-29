//! What the History panel shows, as the frontend receives it.

/// The panel's content: the head's line as interactions, newest first, with the undone ones above the head.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryPanelState {
	pub rows: Vec<HistoryRow>,
	/// Work under way in the room that has not entered history yet, one row per person.
	pub progress: Vec<HistoryProgressRow>,
	/// Whether older interactions exist beyond the rows sent.
	pub more: bool,
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
	/// Branches leaving this interaction that are not on the line shown.
	pub branches: usize,
	/// Above the head: taken back, and redo would bring it back.
	pub undone: bool,
	pub head: bool,
	pub expanded: bool,
	/// The interaction's deltas, newest first, when expanded.
	pub details: Vec<HistoryDeltaRow>,
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
	pub author: String,
	pub anonymous: bool,
	pub mine: bool,
	pub color: String,
	pub ops: usize,
}
