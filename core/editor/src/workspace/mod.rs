pub type PanelId = usize;

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
	// get_serialized_layout()
}

pub struct PanelGroup {
	pub contents: Vec<Contents>,
	pub layout_direction: LayoutDirection,
}

impl PanelGroup {
	fn new() -> PanelGroup {
		PanelGroup {
			contents: vec![],
			layout_direction: LayoutDirection::Horizontal,
		}
	}
}

pub enum Contents {
	PanelArea(PanelArea),
	Group(PanelGroup),
}

pub struct PanelArea {
	pub panels: Vec<PanelId>,
	pub active: PanelId,
}

pub enum LayoutDirection {
	Horizontal,
	Vertical,
}
