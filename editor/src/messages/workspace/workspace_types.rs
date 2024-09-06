use crate::messages::frontend::utility_types::FrontendDocumentDetails;
use crate::messages::portfolio::document::utility_types::misc::DocumentViewId;
use crate::messages::workspace::workspace_message::PortfolioMessageHandler;

use super::workspace_message::DocumentId;

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "tabType", content = "tabData")]
pub enum TabType {
	Layers,
	Properties,
	Document {
		#[serde(rename = "viewId")]
		view_id: DocumentViewId,
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
}
impl TabType {
	pub const fn document(view_id: DocumentViewId, document_id: DocumentId) -> Self {
		Self::Document { view_id, document_id }
	}
}

/// Represents a path to a panel or division
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PanelPath {
	value: u32,
	depth: u32,
}

impl PanelPath {
	/// Construct from a u64 (which is passed to the frontend)
	pub const fn new(value: u64) -> Self {
		let (value, depth) = ((value >> 32) as u32, value as u32);
		Self { value, depth }
	}
	/// Build this panel path to a u64 for passing to the frontend
	pub const fn build(&self) -> u64 {
		((self.value as u64) << 32) + self.depth as u64
	}
	/// A builder for a path - can use `.start()` and `.end()` to append in order (starting from the root division)
	pub const fn builder() -> Self {
		Self { value: 0, depth: 0 }
	}
	/// Get the parent of this path wrapped up as a [`DivisionOrPanel`] and the start (it should be a division but this isn't checked)
	pub fn get_parent_wrapped_mut<'a>(&self, mut root: &'a mut DivisionOrPanel) -> Option<(&'a mut DivisionOrPanel, bool)> {
		let last_shift = self.depth.checked_sub(1)?;
		for shift in 0..last_shift {
			let start = ((self.value >> shift) & 1) == 0;
			root = if start { &mut root.as_division_mut()?.start } else { &mut root.as_division_mut()?.end }
		}
		Some((root, ((self.value >> last_shift) & 1) == 0))
	}
	/// Resolve this path to a [`DivisionOrPanel`] (None for invalid paths)
	pub fn get_wrapped_mut<'a>(&self, mut root: &'a mut DivisionOrPanel) -> Option<&'a mut DivisionOrPanel> {
		for shift in 0..self.depth {
			let start = ((self.value >> shift) & 1) == 0;
			root = if start { &mut root.as_division_mut()?.start } else { &mut root.as_division_mut()?.end }
		}
		Some(root)
	}
	/// Resolve this path to a mutable [`Panel`] (None for divisions or invalid path)
	pub fn get_panel_mut<'a>(&self, root: &'a mut DivisionOrPanel) -> Option<&'a mut Panel> {
		self.get_wrapped_mut(root).and_then(|wrapped| wrapped.as_panel_mut())
	}
	/// Resolve this path to a [`Panel`] (None for divisions or invalid path)
	pub fn get_panel<'a>(&self, mut root: &'a DivisionOrPanel) -> Option<&'a Panel> {
		for shift in 0..self.depth {
			let start = ((self.value >> shift) & 1) == 0;
			root = if start { &root.as_division()?.start } else { &root.as_division()?.end }
		}
		root.as_panel()
	}
	/// Append a start to the end of this path
	pub fn start(mut self) -> Self {
		self.depth += 1;
		self
	}
	/// Append an end to the end of this path
	pub fn end(mut self) -> Self {
		self.value |= 1 << self.depth;
		self.depth += 1;
		self
	}
}

#[test]
fn build_panel_path() {
	let original = PanelPath::builder().end().start().end().end();
	assert_eq!(PanelPath::new(original.build()), original);
}

/// Represents the path to a tab (including its panel and tab index)
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TabPath {
	pub panel: PanelPath,
	pub tab_index: usize,
}

impl TabPath {
	pub fn new(panel: impl Into<PanelPath>, tab_index: usize) -> Self {
		Self { panel: panel.into(), tab_index }
	}
}

/// The orientation and start bool for inserting at the edge to create a new panel - used in the [`TabDestination`]
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct InsertEdge {
	pub direction: Direction,
	pub start: bool,
}

/// Represents the destination for adding or moving a tab
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TabDestination {
	/// The panel for moving to or dividing (if `edge` is set)
	pub panel: PanelPath,
	/// The insert index if inserting into an existing panel (None adds at the end)
	pub insert_index: Option<usize>,
	/// If set, the panel is split and the tab is inserted into the new panel
	pub edge: Option<InsertEdge>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Direction {
	Horizontal,
	Vertical,
}

/// A panel consisting of zero or more tabs, one of which is active.
#[derive(Clone, Debug)]
pub struct Panel {
	tabs: Vec<TabType>,
	active_index: usize,
}

impl Panel {
	pub fn new(tabs: impl Into<Vec<TabType>>) -> Self {
		Self { tabs: tabs.into(), active_index: 0 }
	}
	pub fn add_tab(&mut self, tab: TabType, index: Option<usize>) {
		match index {
			Some(index) => self.tabs.insert(index, tab),
			None => self.tabs.push(tab),
		}
		self.active_index = index.unwrap_or(self.tabs.len().saturating_sub(1))
	}
	pub fn remove_tab(&mut self, index: usize) -> TabType {
		self.active_index = self.active_index.min(self.tabs.len().saturating_sub(2));
		self.tabs.remove(index)
	}
	pub fn is_empty(&self) -> bool {
		self.tabs.is_empty()
	}
	pub fn select_tab(&mut self, index: usize) {
		self.active_index = index.min(self.tabs.len().saturating_sub(1));
	}
	pub fn len(&self) -> usize {
		self.tabs.len()
	}
	pub fn active_index(&self) -> usize {
		self.active_index
	}
	pub fn active_tab(&self) -> Option<TabType> {
		self.tabs.get(self.active_index).copied()
	}
}

#[derive(Clone, Debug)]
pub enum DivisionOrPanel {
	Division(Box<Division>),
	Panel(Panel),
}

impl From<Division> for DivisionOrPanel {
	fn from(value: Division) -> Self {
		Self::Division(Box::new(value))
	}
}

impl From<Panel> for DivisionOrPanel {
	fn from(value: Panel) -> Self {
		Self::Panel(value)
	}
}

impl DivisionOrPanel {
	pub fn as_division(&self) -> Option<&Division> {
		let DivisionOrPanel::Division(division) = self else { return None };
		Some(division)
	}
	pub fn as_division_mut(&mut self) -> Option<&mut Division> {
		let DivisionOrPanel::Division(division) = self else { return None };
		Some(division)
	}
	pub fn as_panel(&self) -> Option<&Panel> {
		let DivisionOrPanel::Panel(panel) = self else { return None };
		Some(panel)
	}
	pub fn as_panel_mut(&mut self) -> Option<&mut Panel> {
		let DivisionOrPanel::Panel(panel) = self else { return None };
		Some(panel)
	}
	/// If this is a division, then replace itself with either its start or end child (as specified)
	pub fn replace_with_child(&mut self, start: bool) {
		let Self::Division(division) = self else { return };
		let child = if start { &mut division.start } else { &mut division.end };
		let child = std::mem::replace(child, Panel::new([]).into());
		*self = child;
	}
	/// Get the first panel by repeatedly getting the start panel of a division; useful for adding tabs with no destination
	pub fn first_panel_mut(&mut self) -> &mut Panel {
		let mut item = self;
		loop {
			match item {
				DivisionOrPanel::Division(division) => item = &mut division.start,
				DivisionOrPanel::Panel(panel) => return panel,
			}
		}
	}
}

#[derive(Clone, Debug)]
pub struct Division {
	direction: Direction,
	start_size: f64,
	end_size: f64,
	start: DivisionOrPanel,
	end: DivisionOrPanel,
}

impl Division {
	pub fn new(direction: Direction, start_size: f64, end_size: f64, start: impl Into<DivisionOrPanel>, end: impl Into<DivisionOrPanel>) -> Self {
		Self {
			direction,
			start_size,
			end_size,
			start: start.into(),
			end: end.into(),
		}
	}
	pub fn start(&self) -> &DivisionOrPanel {
		&self.start
	}
	pub fn end(&self) -> &DivisionOrPanel {
		&self.end
	}
	pub fn direction(&self) -> Direction {
		self.direction
	}
	pub fn resize(&mut self, start_size: f64, end_size: f64) {
		self.start_size = start_size;
		self.end_size = end_size;
	}
	pub fn size(&self) -> [f64; 2] {
		[self.start_size, self.end_size]
	}
}

// ------ Frontend ------ //

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendDivision {
	direction: Direction,
	#[serde(rename = "startSize")]
	start_size: f64,
	#[serde(rename = "endSize")]
	end_size: f64,
	start: FrontendDivisionOrPanel,
	end: FrontendDivisionOrPanel,
	identifier: u64,
}
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FrontendPanel {
	tabs: Vec<TabType>,
	#[serde(rename = "activeIndex")]
	active_index: usize,
	identifier: u64,
}
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FrontendDivisionOrPanel {
	Division(Box<FrontendDivision>),
	Panel(FrontendPanel),
}

impl FrontendPanel {
	pub fn new(source: &Panel, portfolio: &PortfolioMessageHandler, identifier: PanelPath) -> Self {
		Self {
			tabs: source.tabs.clone(),
			//tabs: source.tabs.iter().map(|source| FrontendTabType::new(source, portfolio)).collect(),
			active_index: source.active_index,
			identifier: identifier.build(),
		}
	}
}

impl FrontendDivisionOrPanel {
	pub fn new(source: &DivisionOrPanel, portfolio: &PortfolioMessageHandler, identifier: PanelPath) -> Self {
		match source {
			DivisionOrPanel::Division(source) => Self::Division(Box::new(FrontendDivision::new(source, portfolio, identifier))),
			DivisionOrPanel::Panel(source) => Self::Panel(FrontendPanel::new(source, portfolio, identifier)),
		}
	}
}

impl FrontendDivision {
	pub fn new(source: &Division, portfolio: &PortfolioMessageHandler, identifier: PanelPath) -> Self {
		Self {
			start: FrontendDivisionOrPanel::new(&source.start, portfolio, identifier.start()),
			end: FrontendDivisionOrPanel::new(&source.end, portfolio, identifier.end()),
			direction: source.direction,
			start_size: source.start_size,
			end_size: source.end_size,
			identifier: identifier.build(),
		}
	}
}
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "tabType", content = "tabData")]
pub enum FrontendTabType {
	Layers,
	Properties,
	Document(Option<FrontendDocumentDetails>),
}
impl FrontendTabType {
	pub fn new(source: &TabType, portfolio: &PortfolioMessageHandler) -> Self {
		match source {
			TabType::Layers => Self::Layers,
			TabType::Properties => Self::Properties,
			TabType::Document { view_id, .. } => Self::Document(portfolio.frontend_document(*view_id)),
		}
	}
}
