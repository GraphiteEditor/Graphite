use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::text::{Font, FontCache};

/// Proportional share (0-1) for the document panel's side when splitting adjacent to non-document panels.
const DOCUMENT_PANEL_SHARE: f64 = 0.8;
/// Proportional share for each side when neither (or both) contain the document panel.
const EQUAL_PANEL_SHARE: f64 = 0.5;

#[derive(Debug, Default)]
pub struct CachedData {
	pub font_cache: FontCache,
	pub font_catalog: FontCatalog,
}

// TODO: Should this be a BTreeMap instead?
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalog(pub Vec<FontCatalogFamily>);

impl FontCatalog {
	pub fn find_font_style_in_catalog(&self, font: &Font) -> Option<FontCatalogStyle> {
		let family = self.0.iter().find(|family| family.name == font.font_family);

		let found_style = family.map(|family| {
			let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&font.font_style, "");
			family.closest_style(weight, italic).clone()
		});

		if found_style.is_none() {
			log::warn!("Font not found in catalog: {:?}", font);
		}

		found_style
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogFamily {
	/// The font family name.
	pub name: String,
	/// The font styles (variants) available for the font family.
	pub styles: Vec<FontCatalogStyle>,
}

impl FontCatalogFamily {
	/// Finds the closest style to the given weight and italic setting.
	/// Aims to find the nearest weight while maintaining the italic setting if possible, but italic may change if no other option is available.
	pub fn closest_style(&self, weight: u32, italic: bool) -> &FontCatalogStyle {
		self.styles
			.iter()
			.map(|style| ((style.weight as i32 - weight as i32).unsigned_abs() + 10000 * (style.italic != italic) as u32, style))
			.min_by_key(|(distance, _)| *distance)
			.map(|(_, style)| style)
			.unwrap_or(&self.styles[0])
	}
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FontCatalogStyle {
	pub weight: u32,
	pub italic: bool,
	pub url: String,
}

impl FontCatalogStyle {
	pub fn to_named_style(&self) -> String {
		let weight = self.weight;
		let italic = self.italic;

		let named_weight = Font::named_weight(weight);
		let maybe_italic = if italic { " Italic" } else { "" };

		format!("{named_weight}{maybe_italic} ({weight})")
	}

	pub fn from_named_style(named_style: &str, url: impl Into<String>) -> FontCatalogStyle {
		let weight = named_style.split_terminator(['(', ')']).next_back().and_then(|x| x.parse::<u32>().ok()).unwrap_or(400);
		let italic = named_style.contains("Italic (");
		FontCatalogStyle { weight, italic, url: url.into() }
	}

	/// Get the URL for the stylesheet for loading a font preview for this style of the given family name, subsetted to only the letters in the family name.
	pub fn preview_url(&self, family: impl Into<String>) -> String {
		let name = family.into().replace(' ', "+");
		let italic = if self.italic { "ital," } else { "" };
		let weight = self.weight;
		format!("https://fonts.googleapis.com/css2?display=swap&family={name}:{italic}wght@{weight}&text={name}")
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum PanelType {
	Welcome,
	Document,
	Layers,
	Properties,
	Data,
}

impl From<String> for PanelType {
	fn from(value: String) -> Self {
		match value.as_str() {
			"Welcome" => PanelType::Welcome,
			"Document" => PanelType::Document,
			"Layers" => PanelType::Layers,
			"Properties" => PanelType::Properties,
			"Data" => PanelType::Data,
			_ => panic!("Unknown panel type: {value}"),
		}
	}
}

impl PanelType {
	pub fn non_document_panels() -> &'static [PanelType] {
		&[PanelType::Layers, PanelType::Properties, PanelType::Data]
	}
}

/// Unique identifier for a panel group (a leaf subdivision in the layout tree that holds tabs).
#[repr(transparent)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PanelGroupId(pub u64);

/// Which edge of a panel group to split on when docking a dragged panel.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DockingSplitDirection {
	Left,
	Right,
	Top,
	Bottom,
}

/// State of a single panel group (leaf subdivision) in the workspace layout tree.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PanelGroupState {
	pub tabs: Vec<PanelType>,
	pub active_tab_index: usize,
}

impl PanelGroupState {
	pub fn active_panel_type(&self) -> Option<PanelType> {
		self.tabs.get(self.active_tab_index).copied()
	}

	pub fn contains(&self, panel_type: PanelType) -> bool {
		self.tabs.contains(&panel_type)
	}

	pub fn is_visible(&self, panel_type: PanelType) -> bool {
		self.active_panel_type() == Some(panel_type)
	}
}

/// A subdivision in the workspace layout tree. The root is always a row (horizontal).
/// Direction alternates at each depth: row, column, row, column, etc.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PanelLayoutSubdivision {
	/// A leaf subdivision: a panel group with tabbed panels.
	PanelGroup { id: PanelGroupId, state: PanelGroupState },
	/// A container subdivision that splits its space among children. Direction is implicit from depth (even = row, odd = column).
	Split { children: Vec<SplitChild> },
}

impl Default for PanelLayoutSubdivision {
	fn default() -> Self {
		PanelLayoutSubdivision::Split { children: Vec::new() }
	}
}

/// A child within a split container, with a proportional size weight.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SplitChild {
	pub subdivision: PanelLayoutSubdivision,
	/// Flex-grow weight for proportional sizing.
	pub size: f64,
}

/// Remembers where a panel was before it was removed, so it can be restored to the same location.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct SavedPanelPosition {
	panel_type: PanelType,
	/// The group the panel was a tab in.
	group_id: PanelGroupId,
	/// Which tab index it occupied within that group.
	tab_index: usize,
	/// The group's slot size at the time of removal (used to restore its visual weight via the sibling fallback).
	slot_size: Option<f64>,
	/// When the panel was the sole tab (so the group will be pruned), a neighboring group and
	/// whether to insert before it (`true`) or after it (`false`) to recreate the original position.
	sibling_fallback: Option<(PanelGroupId, bool)>,
}

/// The complete workspace panel layout as a tree of nested rows and columns.
/// The root subdivision is always a row (horizontal split). Direction alternates at each depth.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspacePanelLayout {
	#[serde(default)]
	pub root: PanelLayoutSubdivision,
	/// Counter for generating unique panel group IDs.
	#[serde(default)]
	next_group_id: PanelGroupId,
	/// Remembers where each panel was before removal so it can be restored there.
	#[serde(default)]
	saved_positions: Vec<SavedPanelPosition>,
	/// Whether Focus Document mode is active, hiding all non-document panels.
	#[serde(default)]
	pub focus_document: bool,
}

impl WorkspacePanelLayout {
	/// Generate a new unique panel group ID.
	pub fn next_id(&mut self) -> PanelGroupId {
		let id = self.next_group_id;
		self.next_group_id.0 += 1;
		id
	}

	/// Find the panel group state for a given ID.
	pub fn panel_group(&self, id: PanelGroupId) -> Option<&PanelGroupState> {
		self.root.find_group(id)
	}

	/// Find the panel group state for a given ID (mutable).
	pub fn panel_group_mut(&mut self, id: PanelGroupId) -> Option<&mut PanelGroupState> {
		self.root.find_group_mut(id)
	}

	/// Find which panel group contains a given panel type, returning its ID.
	pub fn find_panel(&self, panel_type: PanelType) -> Option<PanelGroupId> {
		self.root.find_panel(panel_type)
	}

	/// Check if a panel type is the active (visible) tab in any panel group.
	pub fn is_panel_visible(&self, panel_type: PanelType) -> bool {
		self.find_panel(panel_type).and_then(|id| self.panel_group(id)).is_some_and(|group| group.is_visible(panel_type))
	}

	/// Check if a panel type is present (as any tab) in any panel group, whether or not it's the active tab.
	pub fn is_panel_present(&self, panel_type: PanelType) -> bool {
		self.find_panel(panel_type).is_some()
	}

	/// Remove empty panel groups and collapse unnecessary single-child splits.
	pub fn prune(&mut self) {
		self.root.prune();
	}

	/// Produce a filtered copy of this layout containing only the document panel, for use in Focus Document mode.
	pub fn document_only_layout(&self) -> WorkspacePanelLayout {
		let mut layout = self.clone();
		layout.root.retain_only_document_panels();
		layout.root.prune();
		layout
	}

	/// Split a panel group by inserting a new panel group adjacent to it.
	/// The direction determines where the new group goes relative to the target.
	/// Left/Right creates a horizontal (row) split, Top/Bottom creates a vertical (column) split.
	/// Returns the ID of the newly created panel group, or `None` if insertion failed.
	///
	/// `source_slot_size` overrides the new panel's size (for moves where the old slot will be pruned).
	/// If `None`, target's slot is split in the default ratio.
	pub fn split_panel_group(
		&mut self,
		target_group_id: PanelGroupId,
		direction: DockingSplitDirection,
		tabs: Vec<PanelType>,
		active_tab_index: usize,
		source_slot_size: Option<f64>,
	) -> Option<PanelGroupId> {
		let new_id = self.next_id();
		let new_group = SplitChild {
			subdivision: PanelLayoutSubdivision::PanelGroup {
				id: new_id,
				state: PanelGroupState { tabs, active_tab_index },
			},
			size: source_slot_size.unwrap_or(EQUAL_PANEL_SHARE),
		};

		let insert_before = matches!(direction, DockingSplitDirection::Left | DockingSplitDirection::Top);
		let needs_horizontal = matches!(direction, DockingSplitDirection::Left | DockingSplitDirection::Right);

		self.root
			.insert_split_adjacent(target_group_id, new_group, insert_before, needs_horizontal, 0, source_slot_size)
			.then_some(new_id)
	}

	/// Find the slot size of the panel group whose entire content is exactly the given tabs.
	/// Returns `None` if the tabs span multiple groups or don't fill their group exactly.
	pub fn find_source_slot_size(&self, tabs: &[PanelType]) -> Option<f64> {
		if tabs.is_empty() {
			return None;
		}
		let group_id = self.find_panel(tabs[0])?;
		if !tabs.iter().all(|&t| self.find_panel(t) == Some(group_id)) {
			return None;
		}
		let group = self.panel_group(group_id)?;
		if group.tabs.len() != tabs.len() {
			return None;
		}
		self.root.find_slot_size_by_group_id(group_id)
	}

	/// Recalculate the default sizes for all splits in the tree based on document panel proximity.
	pub fn recalculate_default_sizes(&mut self) {
		self.root.recalculate_default_sizes_recursive();
	}

	/// Remember where a panel was before removal so it can be restored there later.
	/// Saves the group ID and tab index. If the panel is the sole tab (so the group will be pruned),
	/// also saves a sibling group as a fallback for adjacency-based restoration.
	pub fn save_panel_position(&mut self, panel_type: PanelType) {
		let Some(group_id) = self.find_panel(panel_type) else { return };
		let tab_index = self.panel_group(group_id).and_then(|g| g.tabs.iter().position(|&t| t == panel_type)).unwrap_or(0);
		let is_sole_tab = self.panel_group(group_id).is_some_and(|g| g.tabs.len() == 1);

		// When it's the sole tab, the group will be pruned, so save a sibling and slot size as fallback
		let sibling_fallback = if is_sole_tab { self.root.find_sibling_group(group_id) } else { None };
		let slot_size = if is_sole_tab { self.root.find_slot_size_by_group_id(group_id) } else { None };

		self.saved_positions.retain(|s| s.panel_type != panel_type);
		self.saved_positions.push(SavedPanelPosition {
			panel_type,
			group_id,
			tab_index,
			slot_size,
			sibling_fallback,
		});
	}

	/// Restore a panel to its previous position if available, otherwise to its default position.
	pub fn restore_panel(&mut self, panel_type: PanelType) {
		let saved = self.saved_positions.iter().find(|s| s.panel_type == panel_type).copied();
		self.saved_positions.retain(|s| s.panel_type != panel_type);

		let Some(saved) = saved else {
			self.restore_panel_to_default_position(panel_type);
			return;
		};

		// Primary: restore as a tab in the original group if it still exists
		if let Some(group) = self.panel_group_mut(saved.group_id) {
			let insert_index = saved.tab_index.min(group.tabs.len());
			group.tabs.insert(insert_index, panel_type);
			group.active_tab_index = insert_index;
			return;
		}

		// Fallback: the original group was pruned, but a sibling in the same parent split survives
		if let Some((sibling_id, before_sibling)) = saved.sibling_fallback
			&& self.root.contains_group(sibling_id)
		{
			let new_id = self.next_id();
			let new_subdivision = PanelLayoutSubdivision::PanelGroup {
				id: new_id,
				state: PanelGroupState {
					tabs: vec![panel_type],
					active_tab_index: 0,
				},
			};

			let new_group = SplitChild {
				subdivision: new_subdivision,
				size: saved.slot_size.unwrap_or_else(|| {
					let sibling_is_document_panel = self.root.find_group(sibling_id).is_some_and(|g| g.contains(PanelType::Document) || g.contains(PanelType::Welcome));
					if sibling_is_document_panel { 1. - DOCUMENT_PANEL_SHARE } else { EQUAL_PANEL_SHARE }
				}),
			};
			self.root.insert_adjacent_to_group(sibling_id, new_group, before_sibling);
			return;
		}

		self.restore_panel_to_default_position(panel_type);
	}

	/// Place a panel at its default structural position in the layout tree.
	/// - Data: below the document in the left column (root child 0)
	/// - Properties: top of the right column (root child 1)
	/// - Layers: bottom of the right column (root child 1)
	fn restore_panel_to_default_position(&mut self, panel_type: PanelType) {
		let new_id = self.next_id();
		let new_group = SplitChild {
			subdivision: PanelLayoutSubdivision::PanelGroup {
				id: new_id,
				state: PanelGroupState {
					tabs: vec![panel_type],
					active_tab_index: 0,
				},
			},
			size: EQUAL_PANEL_SHARE,
		};

		// Determine which root child column to insert into and at which position
		let (root_child_index, insert_at_end) = match panel_type {
			PanelType::Data => (0, true),        // Left column, after document
			PanelType::Properties => (1, false), // Right column, at top
			PanelType::Layers => (1, true),      // Right column, at bottom
			_ => (1, true),
		};

		// Ensure the root is a split
		if !matches!(&self.root, PanelLayoutSubdivision::Split { .. }) {
			let old_root = std::mem::replace(&mut self.root, PanelLayoutSubdivision::Split { children: vec![] });
			if let PanelLayoutSubdivision::Split { children } = &mut self.root {
				children.push(SplitChild {
					subdivision: old_root,
					size: DOCUMENT_PANEL_SHARE,
				});
			}
		}

		let PanelLayoutSubdivision::Split { children: root_children } = &mut self.root else { return };

		// Ensure the target root child exists
		while root_children.len() <= root_child_index {
			root_children.push(SplitChild {
				subdivision: PanelLayoutSubdivision::Split { children: vec![] },
				size: (1. - DOCUMENT_PANEL_SHARE),
			});
		}

		// The target should be a split (column at depth 1) so we can add children to it
		let target = &mut root_children[root_child_index].subdivision;
		if !matches!(target, PanelLayoutSubdivision::Split { .. }) {
			let old_subdivision = std::mem::replace(target, PanelLayoutSubdivision::Split { children: vec![] });
			if let PanelLayoutSubdivision::Split { children } = target {
				children.push(SplitChild {
					subdivision: old_subdivision,
					size: EQUAL_PANEL_SHARE,
				});
			}
		}

		if let PanelLayoutSubdivision::Split { children } = target {
			if insert_at_end {
				children.push(new_group);
			} else {
				children.insert(0, new_group);
			}
		}

		// Recalculate sizes within the target column to get the correct document-aware ratio
		let PanelLayoutSubdivision::Split { children: root_children } = &mut self.root else { return };
		if let Some(target) = root_children.get_mut(root_child_index) {
			target.subdivision.recalculate_default_sizes();
		}
	}
}

impl Default for WorkspacePanelLayout {
	fn default() -> Self {
		// Default layout (sizes are recalculated by `recalculate_default_sizes` before being sent to the frontend):
		// Row [
		//   Column [Document]
		//   Column [Properties, Layers]
		// ]
		Self {
			root: PanelLayoutSubdivision::Split {
				children: vec![
					SplitChild {
						subdivision: PanelLayoutSubdivision::Split {
							children: vec![SplitChild {
								subdivision: PanelLayoutSubdivision::PanelGroup {
									id: PanelGroupId(0),
									state: PanelGroupState {
										tabs: vec![PanelType::Document],
										active_tab_index: 0,
									},
								},
								size: 1.,
							}],
						},
						size: DOCUMENT_PANEL_SHARE,
					},
					SplitChild {
						subdivision: PanelLayoutSubdivision::Split {
							children: vec![
								SplitChild {
									subdivision: PanelLayoutSubdivision::PanelGroup {
										id: PanelGroupId(1),
										state: PanelGroupState {
											tabs: vec![PanelType::Properties],
											active_tab_index: 0,
										},
									},
									size: EQUAL_PANEL_SHARE,
								},
								SplitChild {
									subdivision: PanelLayoutSubdivision::PanelGroup {
										id: PanelGroupId(2),
										state: PanelGroupState {
											tabs: vec![PanelType::Layers],
											active_tab_index: 0,
										},
									},
									size: EQUAL_PANEL_SHARE,
								},
							],
						},
						size: (1. - DOCUMENT_PANEL_SHARE),
					},
				],
			},
			next_group_id: PanelGroupId(3),
			saved_positions: Vec::new(),
			focus_document: false,
		}
	}
}

/// The share of the slot that should go to the old side when splitting it with a new side.
fn document_split_share(old_side: &PanelLayoutSubdivision, new_side: &PanelLayoutSubdivision) -> f64 {
	match (old_side.contains_document(), new_side.contains_document()) {
		(true, false) => DOCUMENT_PANEL_SHARE,
		(false, true) => 1. - DOCUMENT_PANEL_SHARE,
		_ => EQUAL_PANEL_SHARE,
	}
}

impl PanelLayoutSubdivision {
	/// Find the panel group state for a given ID.
	pub fn find_group(&self, target_id: PanelGroupId) -> Option<&PanelGroupState> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if *id == target_id => Some(state),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter().find_map(|child| child.subdivision.find_group(target_id)),
		}
	}

	/// Find the panel group state for a given ID (mutable).
	pub fn find_group_mut(&mut self, target_id: PanelGroupId) -> Option<&mut PanelGroupState> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if *id == target_id => Some(state),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter_mut().find_map(|child| child.subdivision.find_group_mut(target_id)),
		}
	}

	/// Find the panel group ID that contains a given panel type.
	pub fn find_panel(&self, panel_type: PanelType) -> Option<PanelGroupId> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, state } if state.contains(panel_type) => Some(*id),
			PanelLayoutSubdivision::PanelGroup { .. } => None,
			PanelLayoutSubdivision::Split { children } => children.iter().find_map(|child| child.subdivision.find_panel(panel_type)),
		}
	}

	/// Collect all panel group IDs in the tree.
	pub fn all_group_ids(&self) -> Vec<PanelGroupId> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, .. } => vec![*id],
			PanelLayoutSubdivision::Split { children } => children.iter().flat_map(|child| child.subdivision.all_group_ids()).collect(),
		}
	}

	/// Remove empty groups/splits and flatten single-child `Split`-in-`Split` nesting (which docking sequences can create).
	///
	/// Flattening preserves depth parity (and therefore direction). `PanelGroup`-only single-child splits are left
	/// alone since collapsing would change the panel's depth and alter future wrap orientation.
	pub fn prune(&mut self) {
		let PanelLayoutSubdivision::Split { children } = self else { return };

		// Recursively prune children
		children.iter_mut().for_each(|child| child.subdivision.prune());

		// Remove empty panel groups
		children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { state, .. } if state.tabs.is_empty()));

		// Remove empty splits (splits that lost all their children after pruning)
		children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::Split { children } if children.is_empty()));

		// Flatten single-child `Split`-in-`Split` nesting, rescaling sizes to preserve visual proportions
		let mut i = 0;
		while i < children.len() {
			// Must be a `Split`...
			let PanelLayoutSubdivision::Split { children: outer } = &children[i].subdivision else {
				i += 1;
				continue;
			};
			// ...with exactly one child...
			let [only_child] = outer.as_slice() else {
				i += 1;
				continue;
			};
			// ...that is itself a `Split`
			let PanelLayoutSubdivision::Split { .. } = &only_child.subdivision else {
				i += 1;
				continue;
			};

			// Remove the redundant wrapper
			let removed = children.remove(i);
			let outer_size = removed.size;

			// Extract the inner grandchildren
			let PanelLayoutSubdivision::Split { children: mut outer_children } = removed.subdivision else {
				continue;
			};
			let Some(inner_split) = outer_children.pop() else { continue };
			let PanelLayoutSubdivision::Split { children: inner_children } = inner_split.subdivision else {
				continue;
			};

			// Splice grandchildren in at the same position, scaling their sizes to fill the removed slot
			let inner_total: f64 = inner_children.iter().map(|c| c.size).sum();
			for (offset, mut grandchild) in inner_children.into_iter().enumerate() {
				grandchild.size = if inner_total > 0. { grandchild.size / inner_total * outer_size } else { outer_size };
				children.insert(i + offset, grandchild);
			}
		}

		// Renormalize to sum=1 since dock/prune cycles can compound shrinkage
		let total: f64 = children.iter().map(|c| c.size).sum();
		if total > 0. && (total - 1.).abs() > 0.001 {
			for child in children.iter_mut() {
				child.size /= total;
			}
		}
	}

	/// Remove all non-document/non-welcome tabs from panel groups, leaving only document-related panels.
	pub fn retain_only_document_panels(&mut self) {
		match self {
			PanelLayoutSubdivision::PanelGroup { state, .. } => {
				state.tabs.retain(|t| matches!(t, PanelType::Document | PanelType::Welcome));
				state.active_tab_index = state.active_tab_index.min(state.tabs.len().saturating_sub(1));
			}
			PanelLayoutSubdivision::Split { children } => {
				children.iter_mut().for_each(|child| child.subdivision.retain_only_document_panels());
			}
		}
	}

	/// Find the nearest sibling panel group for the given group within the same parent split.
	/// Returns the sibling's ID and whether the target was before it (`true`) or after it (`false`).
	/// Prefers the immediately previous sibling (with before=false meaning "insert after it"), falling
	/// back to the next sibling (with before=true) so all positions in a 3-child split are distinguishable.
	pub fn find_sibling_group(&self, target_id: PanelGroupId) -> Option<(PanelGroupId, bool)> {
		let PanelLayoutSubdivision::Split { children } = self else { return None };

		let target_index = children
			.iter()
			.position(|child| matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == target_id));

		if let Some(index) = target_index {
			let previous = (0..index).rev().find_map(|i| Self::group_id_of(&children[i]).map(|id| (id, false)));
			let next = ((index + 1)..children.len()).find_map(|i| Self::group_id_of(&children[i]).map(|id| (id, true)));
			return previous.or(next);
		}

		children.iter().find_map(|child| child.subdivision.find_sibling_group(target_id))
	}

	/// Get a panel group ID from a child, either directly or the first one in a subtree.
	fn group_id_of(child: &SplitChild) -> Option<PanelGroupId> {
		match &child.subdivision {
			PanelLayoutSubdivision::PanelGroup { id, .. } => Some(*id),
			sub => sub.first_group_id(),
		}
	}

	/// Return the first panel group ID found in this subtree.
	fn first_group_id(&self) -> Option<PanelGroupId> {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, .. } => Some(*id),
			PanelLayoutSubdivision::Split { children } => children.iter().find_map(|child| child.subdivision.first_group_id()),
		}
	}

	/// Insert a new split child immediately before or after the given group in its parent split,
	/// scaling existing siblings down proportionally to make room for the new child's size.
	pub fn insert_adjacent_to_group(&mut self, sibling_id: PanelGroupId, new_child: SplitChild, before_sibling: bool) {
		let PanelLayoutSubdivision::Split { children } = self else { return };

		let sibling_index = children
			.iter()
			.position(|child| matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == sibling_id));
		if let Some(index) = sibling_index {
			// Shrink existing siblings proportionally to make room
			let old_total: f64 = children.iter().map(|c| c.size).sum();
			let scale = if old_total > 0. { (old_total - new_child.size).max(0.) / old_total } else { 1. };
			for child in children.iter_mut() {
				child.size *= scale;
			}

			let insert_at = if before_sibling { index } else { index + 1 };
			children.insert(insert_at, new_child);
			return;
		}

		for child in children.iter_mut() {
			if child.subdivision.contains_group(sibling_id) {
				child.subdivision.insert_adjacent_to_group(sibling_id, new_child, before_sibling);
				return;
			}
		}
	}

	/// Check if this subtree contains a panel group with the given ID.
	pub fn contains_group(&self, target_id: PanelGroupId) -> bool {
		match self {
			PanelLayoutSubdivision::PanelGroup { id, .. } => *id == target_id,
			PanelLayoutSubdivision::Split { children } => children.iter().any(|child| child.subdivision.contains_group(target_id)),
		}
	}

	/// Inserts a new split child adjacent to a target panel group and returns whether the insertion was successful.
	/// Recurses to the deepest split closest to the target that matches the requested split direction.
	/// If the target is a direct child of a mismatched-direction split, this wraps it in a new sub-split.
	///
	/// `source_slot_size` preserves the moved panel's visual weight. If `None`, uses the default split ratio.
	pub fn insert_split_adjacent(&mut self, target_id: PanelGroupId, new_child: SplitChild, insert_before: bool, needs_horizontal: bool, depth: usize, source_slot_size: Option<f64>) -> bool {
		let PanelLayoutSubdivision::Split { children } = self else { return false };

		let is_horizontal = depth.is_multiple_of(2);
		let direction_matches = is_horizontal == needs_horizontal;

		// Find which child subtree contains the target
		let Some(containing_index) = children.iter().position(|child| child.subdivision.contains_group(target_id)) else {
			return false;
		};

		// If the target is a direct child: we can certainly insert the new split, either as a sibling (if direction matches) or wrapping the target in a new split (if direction is mismatched)
		let target_is_direct_child = matches!(&children[containing_index].subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == target_id);
		if target_is_direct_child {
			// Direction matches: insert as sibling, sizing based on whether target will be pruned, source size hint, or default ratio
			if direction_matches {
				let mut new_child = new_child;
				let target_will_be_pruned = matches!(&children[containing_index].subdivision, PanelLayoutSubdivision::PanelGroup { state, .. } if state.tabs.is_empty());
				if target_will_be_pruned {
					new_child.size = children[containing_index].size;
				} else if let Some(hint) = source_slot_size {
					new_child.size = hint;
				} else {
					let target_share = document_split_share(&children[containing_index].subdivision, &new_child.subdivision);
					let total = children[containing_index].size;
					children[containing_index].size = total * target_share;
					new_child.size = total * (1. - target_share);
				}

				let insert_index = if insert_before { containing_index } else { containing_index + 1 };
				children.insert(insert_index, new_child);
			}
			// Direction mismatch: wrap target in a sub-split at depth+1, sharing the slot in the default ratio
			else {
				let old_child_subdivision = std::mem::replace(&mut children[containing_index].subdivision, PanelLayoutSubdivision::Split { children: vec![] });

				let old_share = document_split_share(&old_child_subdivision, &new_child.subdivision);
				let old_child = SplitChild {
					subdivision: old_child_subdivision,
					size: old_share,
				};
				let mut new_child = new_child;
				new_child.size = 1. - old_share;

				if let PanelLayoutSubdivision::Split { children: sub_children } = &mut children[containing_index].subdivision {
					if insert_before {
						sub_children.push(new_child);
						sub_children.push(old_child);
					} else {
						sub_children.push(old_child);
						sub_children.push(new_child);
					}
				}
			}

			return true;
		}

		// The target is deeper, so recurse into the containing child's subtree and return its insertion outcome
		children[containing_index]
			.subdivision
			.insert_split_adjacent(target_id, new_child.clone(), insert_before, needs_horizontal, depth + 1, source_slot_size)
	}

	/// Find the size of the `SplitChild` slot whose subdivision is the panel group with the given ID, if it exists.
	pub fn find_slot_size_by_group_id(&self, group_id: PanelGroupId) -> Option<f64> {
		let PanelLayoutSubdivision::Split { children } = self else { return None };
		for child in children {
			if let PanelLayoutSubdivision::PanelGroup { id, .. } = &child.subdivision
				&& *id == group_id
			{
				return Some(child.size);
			}
		}
		for child in children {
			if let Some(size) = child.subdivision.find_slot_size_by_group_id(group_id) {
				return Some(size);
			}
		}
		None
	}

	/// Check if this subtree contains the document panel.
	pub fn contains_document(&self) -> bool {
		match self {
			PanelLayoutSubdivision::PanelGroup { state, .. } => state.contains(PanelType::Document) || state.contains(PanelType::Welcome),
			PanelLayoutSubdivision::Split { children } => children.iter().any(|child| child.subdivision.contains_document()),
		}
	}

	/// Recalculate the default sizes for this subdivision's direct children based on proximity to the document panel.
	/// Splits directly surrounding the document panel use 80-20 weighting.
	/// All other splits use equal division.
	/// Does not recurse into descendants: use [`Self::recalculate_default_sizes_recursive`] for that.
	pub fn recalculate_default_sizes(&mut self) {
		let PanelLayoutSubdivision::Split { children } = self else { return };

		let child_count = children.len();
		if child_count == 0 {
			return;
		}

		// Check if any child directly contains (or is) the document panel
		let document_child_index = children.iter().position(|child| child.subdivision.contains_document());

		if let Some(document_index) = document_child_index {
			// This split directly surrounds the document panel
			let non_document_count = child_count - 1;
			let document_share = if non_document_count > 0 { DOCUMENT_PANEL_SHARE } else { 1. };
			let other_share = if non_document_count > 0 { (1. - DOCUMENT_PANEL_SHARE) / non_document_count as f64 } else { 0. };

			for (i, child) in children.iter_mut().enumerate() {
				child.size = if i == document_index { document_share } else { other_share };
			}
		} else {
			// This split doesn't directly contain the document, use equal division
			let equal_share = 1. / child_count as f64;
			for child in children.iter_mut() {
				child.size = equal_share;
			}
		}
	}

	/// Recalculate the default sizes for this subdivision and all its descendant splits.
	pub fn recalculate_default_sizes_recursive(&mut self) {
		self.recalculate_default_sizes();

		if let PanelLayoutSubdivision::Split { children } = self {
			for child in children.iter_mut() {
				child.subdivision.recalculate_default_sizes_recursive();
			}
		}
	}

	/// Remove a panel group by ID from the tree. Does not prune.
	pub fn remove_group(&mut self, target_id: PanelGroupId) {
		if let PanelLayoutSubdivision::Split { children } = self {
			children.retain(|child| !matches!(&child.subdivision, PanelLayoutSubdivision::PanelGroup { id, .. } if *id == target_id));

			children.iter_mut().for_each(|child| child.subdivision.remove_group(target_id));
		}
	}
}

pub enum FileContent {
	/// A Graphite document.
	Document(String),
	/// A bitmap image.
	Image(Image<Color>),
	/// An SVG file string.
	Svg(String),
	/// Any other unsupported/unrecognized file type.
	Unsupported,
}
