//! What the History panel shows, as the frontend receives it, and the changes it asks of the head's line.

use glam::DVec2;
use graph_craft::document::NodeId;
use std::collections::HashMap;

/// The document as rendered once a step is done, for the History panel to show and compare.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentCapture {
	pub svg: String,
	/// The document area the SVG shows.
	pub frame: [DVec2; 2],
	/// The proto node giving each layer's own content, whose ID changes with that content.
	pub layer_sources: HashMap<NodeId, NodeId>,
}

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
	/// Others are connected, so authors, whether work is in history yet and head moves concern them too.
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
	/// How the document looked once this step was done, if the panel was open to capture it.
	pub thumbnail: Option<HistoryThumbnail>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryThumbnail {
	pub after: HistoryShot,
	/// The step before, when it was captured too, outlining what this step took away or changed.
	pub before: Option<HistoryShot>,
}

/// One captured image, whose SVG the frontend received under `id`, with outlines in document coordinates.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryShot {
	pub id: String,
	/// The document area shown, as x, y, width and height.
	pub frame: [f64; 4],
	pub outlines: Vec<HistoryOutline>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryOutline {
	pub change: OutlineChange,
	pub x: f64,
	pub y: f64,
	pub width: f64,
	pub height: f64,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OutlineChange {
	Added,
	Removed,
	Changed,
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
