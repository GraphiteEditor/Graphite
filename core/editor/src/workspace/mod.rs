use serde::{Deserialize, Serialize};

pub type PanelId = u32;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
	pub hovered_panel: PanelId,
	pub root: PanelGroup,
}

impl Workspace {
	pub fn new() -> Workspace {
		Workspace {
			hovered_panel: 0,
			root: PanelGroup::new(),
		}
	}
	// add panel / panel group
	// delete panel / panel group
	// move panel / panel group
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PanelGroup {
	pub contents: Vec<Contents>,
	pub layout_direction: LayoutDirection,
}

impl Default for PanelGroup {
	fn default() -> Self {
		Self::new()
	}
}

impl PanelGroup {
	fn new() -> PanelGroup {
		PanelGroup {
			contents: vec![],
			layout_direction: LayoutDirection::Horizontal,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Contents {
	PanelArea(PanelArea),
	Group(PanelGroup),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PanelArea {
	pub panels: Vec<PanelId>,
	pub active: PanelId,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LayoutDirection {
	Horizontal,
	Vertical,
}
