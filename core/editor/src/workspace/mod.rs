use crate::EditorError;
pub type PanelId = usize;

pub struct Workspace {
	hovered_panel: PanelId,
	root: PanelGroup,
}

impl Workspace {
	// add panel / panel group
	// delete panel / panel group
	// move panel / panel group
	// get_serialized_layout()
}

struct PanelGroup {
	contents: Vec<Contents>,
	layout_direction: LayoutDirection,
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
