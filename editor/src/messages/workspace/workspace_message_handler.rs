use crate::messages::prelude::*;

pub use super::workspace_types::*;

#[derive(Debug, Clone)]
pub struct WorkspaceMessageHandler {
	root: DivisionOrPanel,
}

impl Default for WorkspaceMessageHandler {
	fn default() -> Self {
		let documents = Panel::new([]);
		let properties = Panel::new([TabType::Properties]);
		let layers = Panel::new([TabType::Layers, TabType::Properties, TabType::Layers]);
		let right = Division::new(Direction::Vertical, 45., 55., properties, layers);
		Self {
			root: Division::new(Direction::Horizontal, 80., 20., documents, right).into(),
		}
	}
}

impl WorkspaceMessageHandler {
	pub fn add_tab(&mut self, panel: impl Into<PanelPath>, tab: TabType, index: Option<usize>) {
		let Some(panel) = panel.into().get_panel_mut(&mut self.root) else {
			warn!("Invalid panel for add");
			return;
		};
		panel.add_tab(tab, index);
	}
	pub fn delete_tab(&mut self, tab: TabPath) {
		let Some(panel) = tab.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid panel for delete");
			return;
		};
		panel.remove_tab(tab.tab_index);
		if panel.is_empty() {
			if let Some((parent, start)) = tab.panel.get_parent_mut(&mut self.root) {
				parent.replace_with_child(!start);
			}
		}
	}
	pub fn move_tab(&mut self, source: TabPath, destination: TabDestination) {
		let Some(panel) = source.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid source panel for move");
			return;
		};

		// Don't bother
		if destination.panel == source.panel && panel.len() == 1 {
			return;
		}

		let removed = panel.remove_tab(source.tab_index);
		let remove_division = panel.is_empty();
		if let Some(edge) = destination.edge {
			let Some(destination_wrapper) = destination.panel.get_wrapped_mut(&mut self.root) else {
				warn!("Invalid destination panel for move");
				return;
			};
			let opposite = std::mem::replace(destination_wrapper, Panel::new([]).into());
			let new_panel = Panel::new([removed]).into();
			let [start, end] = if edge.start { [new_panel, opposite] } else { [opposite, new_panel] };
			*destination_wrapper = Division::new(edge.direction, 50., 50., start, end).into();
		} else {
			let Some(destination_panel) = destination.panel.get_panel_mut(&mut self.root) else {
				warn!("Invalid destination panel for move");
				return;
			};
			destination_panel.add_tab(removed, destination.insert_index);
		}

		if remove_division {
			if let Some((parent, start)) = source.panel.get_parent_mut(&mut self.root) {
				parent.replace_with_child(!start);
			}
		}
	}
	pub fn select_tab(&mut self, tab: TabPath) {
		let Some(panel) = tab.panel.get_panel_mut(&mut self.root) else {
			warn!("Invalid panel for select");
			return;
		};
		panel.select_tab(tab.tab_index);
	}
	pub fn resize_division(&mut self, division: PanelPath, start_size: f64, end_size: f64) {
		let Some(division) = division.get_wrapped_mut(&mut self.root).and_then(|wrapped| wrapped.as_division_mut()) else {
			warn!("Invalid division for resize");
			return;
		};
		division.resize(start_size, end_size);
	}
	pub fn is_single_tab(&self, panel: PanelPath) -> bool {
		let Some(panel) = panel.get_panel(&self.root) else {
			warn!("Invalid panel for check single tab");
			return true;
		};
		panel.len() == 1
	}

	pub fn send_layout(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateDockspace {
			root: FrontendDivisionOrPanel::new(&self.root, PanelPath::builder()),
		});
	}
}

impl MessageHandler<WorkspaceMessage, ()> for WorkspaceMessageHandler {
	fn process_message(&mut self, message: WorkspaceMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			WorkspaceMessage::AddTab { panel, tab, index } => self.add_tab(panel, tab, index),
			WorkspaceMessage::DeleteTab { tab } => self.delete_tab(tab),
			WorkspaceMessage::MoveTab { source, destination } => self.move_tab(source, destination),
			WorkspaceMessage::SelectTab { tab } => self.select_tab(tab),
			WorkspaceMessage::ResizeDivision { division, start_size, end_size } => self.resize_division(division, start_size, end_size),

			WorkspaceMessage::SendLayout => {} // Send layout is called anyway below.
		}
		self.send_layout(responses);
	}

	fn actions(&self) -> ActionList {
		actions!(WorkspaceMessageDiscriminant;)
	}
}

#[cfg(test)]
use crate::messages::portfolio::document::utility_types::misc::DocumentViewId;

#[cfg(test)]
fn test_layout() -> WorkspaceMessageHandler {
	let documents = Panel::new([]);
	let end_start = Panel::new([TabType::Document(DocumentViewId(1_000))]);
	let end_end = Panel::new([TabType::Document(DocumentViewId(11_000)), TabType::Document(DocumentViewId(11_001))]);
	let right = Division::new(Direction::Vertical, 45., 55., end_start, end_end);
	WorkspaceMessageHandler {
		root: Division::new(Direction::Horizontal, 80., 20., documents, right).into(),
	}
}

#[cfg(test)]
const TEST_TAB: TabType = TabType::Document(DocumentViewId(123));

#[test]
fn dockspace_add_tab() {
	let mut workspace = test_layout();
	workspace.add_tab(PanelPath::builder().end().start(), TEST_TAB, None);
	let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().start().as_panel().unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_insert_tab() {
	let mut workspace = test_layout();
	workspace.add_tab(PanelPath::builder().end().end(), TEST_TAB, Some(1));
	let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 3);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_insert_tab_empty() {
	let mut workspace = test_layout();
	workspace.add_tab(PanelPath::builder().start(), TEST_TAB, Some(0));
	let panel = workspace.root.as_division().unwrap().start().as_panel().unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_index(), 0);
	assert_eq!(panel.active_tab().unwrap(), TEST_TAB);
}

#[test]
fn dockspace_delete_tab() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 1));
	let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_000)));
}

#[test]
fn dockspace_delete_replace_active() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_001)));
}

#[test]
fn dockspace_delete_division() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	let panel = workspace.root.as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(1_000)));
}

#[test]
fn dockspace_delete_2_divisions() {
	let mut workspace = test_layout();
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end().end(), 0));
	workspace.delete_tab(TabPath::new(PanelPath::builder().end(), 0));
	let panel = workspace.root.as_panel().unwrap();
	assert_eq!(panel.len(), 0);
	assert!(panel.active_tab().is_none());
}

#[test]
fn dockspace_select_tab() {
	let mut workspace = test_layout();
	workspace.select_tab(TabPath::new(PanelPath::builder().end().end(), 1));
	let panel = PanelPath::builder().end().end().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_001)));
}

#[test]
fn dockspace_move_tab_simple() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().start(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 1), destination);
	let panel = PanelPath::builder().end().end().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	let panel = PanelPath::builder().end().start().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_001)));
}

#[test]
fn dockspace_move_tab_on_self() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().start(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
	let panel = PanelPath::builder().end().start().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(1_000)));
}

#[test]
fn dockspace_move_tab_on_self_stack() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().end(),
		insert_index: None,
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
	let panel = PanelPath::builder().end().end().get_panel_mut(&mut workspace.root).unwrap();
	assert_eq!(panel.len(), 2);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_000)));
}

#[test]
fn dockspace_move_tab_delete_divison() {
	let mut workspace = test_layout();
	let destination = TabDestination {
		panel: PanelPath::builder().end().end(),
		insert_index: Some(1),
		edge: None,
	};
	workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
	let panel = workspace.root.as_division().unwrap().end().as_panel().unwrap();
	assert_eq!(panel.len(), 3);
	assert_eq!(panel.active_index(), 1);
	assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(1_000)));
}

#[test]
fn dockspace_move_tab_edge() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().start(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap().start().as_division().unwrap();
		assert_eq!(division.direction(), direction);
		let start_panel = division.start().as_panel().unwrap();
		let end_panel = division.end().as_panel().unwrap();
		assert_eq!(start_panel.len(), 1);
		assert_eq!(end_panel.len(), 1);
		assert_eq!(start_panel.active_tab().unwrap(), TabType::Document(DocumentViewId(if start { 11_000 } else { 1_000 })));
		assert_eq!(end_panel.active_tab().unwrap(), TabType::Document(DocumentViewId(if start { 1_000 } else { 11_000 })));
	}
}

#[test]
fn dockspace_move_tab_edge_delete_other() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().end(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap();
		assert_eq!(division.direction(), direction);

		let [insert, rest] = if start { [division.start(), division.end()] } else { [division.end(), division.start()] };
		let insert_panel = insert.as_panel().unwrap();
		let rest_panel = rest.as_panel().unwrap();
		assert_eq!(insert_panel.len(), 1);
		assert_eq!(rest_panel.len(), 2);
		assert_eq!(insert_panel.active_tab().unwrap(), TabType::Document(DocumentViewId(1_000)));
		assert_eq!(rest_panel.active_tab().unwrap(), TabType::Document(DocumentViewId(11_000)));
	}
}

#[test]
fn dockspace_can_not_edge_self() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().start(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().start(), 0), destination);
		let panel = workspace.root.as_division().unwrap().end().as_division().unwrap().start().as_panel().unwrap();
		assert_eq!(panel.len(), 1);
		assert_eq!(panel.active_tab().unwrap(), TabType::Document(DocumentViewId(1_000)));
	}
}

#[test]
fn dockspace_can_edge_self_with_other_tabs() {
	for (direction, start) in [(Direction::Horizontal, true), (Direction::Horizontal, false), (Direction::Vertical, true), (Direction::Vertical, false)] {
		let mut workspace = test_layout();
		let destination = TabDestination {
			panel: PanelPath::builder().end().end(),
			insert_index: None,
			edge: Some(InsertEdge { direction, start }),
		};
		workspace.move_tab(TabPath::new(PanelPath::builder().end().end(), 0), destination);
		let division = workspace.root.as_division().unwrap().end().as_division().unwrap().end().as_division().unwrap();
		let [moved, other] = if start { [division.start(), division.end()] } else { [division.end(), division.start()] };
		assert_eq!(moved.as_panel().unwrap().active_tab().unwrap(), TabType::Document(DocumentViewId(11_000)));
		assert_eq!(other.as_panel().unwrap().active_tab().unwrap(), TabType::Document(DocumentViewId(11_001)));
	}
}

#[test]
fn dockspace_resize() {
	let mut workspace = test_layout();

	workspace.resize_division(PanelPath::builder().end(), 11., 13.);
	let division = workspace.root.as_division().unwrap().end().as_division().unwrap();
	assert_eq!(division.size(), (11., 13.))
}
