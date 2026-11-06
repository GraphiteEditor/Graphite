//! What the History panel shows, as the frontend receives it, and the changes it asks of the head's line.

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LineChange {
	/// Moves the shared head to an earlier interaction and keeps the later steps as a branch.
	MoveHead,
	/// Drops a retired interaction and mints the later ones again on its parent.
	Remove,
	/// Does a step off the line again on top of it.
	Restore,
}

/// The panel's content, newest first. The head's line with undone interactions above it, or a followed branch's line.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryPanelState {
	pub rows: Vec<HistoryRow>,
	/// Work not yet in history. One row per person's open work and per closed interaction waiting to retire.
	pub progress: Vec<HistoryProgressRow>,
	pub more: bool,
	pub following: Option<String>,
	/// Others are connected, so authors, work in progress and head moves concern them too.
	pub collaborating: bool,
}

/// One interaction, a run of deltas that undo takes back together.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryRow {
	/// The rev closing the interaction, in decimal.
	pub id: String,
	pub label: String,
	pub named: bool,
	pub tags: Vec<String>,
	pub author: String,
	/// `author` is a stand-in.
	pub anonymous: bool,
	pub mine: bool,
	pub color: String,
	/// Milliseconds since the Unix epoch, if the retirer recorded it.
	pub time: Option<f64>,
	pub deltas: usize,
	/// Branches off the shown line, each by its tip.
	pub branches: Vec<HistoryBranch>,
	pub undone: bool,
	/// On a followed branch, off the head's line.
	pub abandoned: bool,
	pub head: bool,
	pub expanded: bool,
	pub details: Vec<HistoryDeltaRow>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryBranch {
	pub id: String,
	pub label: String,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryDeltaRow {
	pub id: String,
	pub label: String,
	pub author: String,
	pub mine: bool,
	pub time: Option<f64>,
}

/// A person with unretired work in the room's hot log.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryProgressRow {
	pub label: String,
	pub author: String,
	pub anonymous: bool,
	pub mine: bool,
	pub color: String,
	pub ops: usize,
	/// Still being made, not closed and waiting.
	pub open: bool,
}
