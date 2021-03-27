use crate::EditorError;
pub type PanelId = usize;

pub struct Workspace {
	hovered_panel: PanelId,
	root: PanelGroup,
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

struct PanelGroup {
	contents: Vec<Contents>,
	layout_direction: LayoutDirection,
}

impl PanelGroup {
	fn new() -> PanelGroup {
		PanelGroup {
			contents: vec![],
			layout_direction: LayoutDirection::Horizontal,
		}
	}
}

enum Contents {
	PanelArea(PanelArea),
	Group(PanelGroup),
}

struct PanelArea {
	panels: Vec<PanelId>,
	active: PanelId,
}

enum LayoutDirection {
	Horizontal,
	Vertical,
}
