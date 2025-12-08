use super::widgets::button_widgets::*;
use super::widgets::input_widgets::*;
use super::widgets::label_widgets::*;
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::prelude::*;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WidgetId(pub u64);

impl core::fmt::Display for WidgetId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[derive(PartialEq, Clone, Debug, Hash, Eq, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[repr(u8)]
pub enum LayoutTarget {
	/// The spreadsheet panel allows for the visualisation of data in the graph.
	DataPanel,
	/// Contains the action buttons at the bottom of the dialog. Must be shown with the `FrontendMessage::DisplayDialog` message.
	DialogButtons,
	/// Contains the contents of the dialog's primary column. Must be shown with the `FrontendMessage::DisplayDialog` message.
	DialogColumn1,
	/// Contains the contents of the dialog's secondary column (often blank). Must be shown with the `FrontendMessage::DisplayDialog` message.
	DialogColumn2,
	/// Contains the widgets located directly above the canvas to the right, for example the zoom in and out buttons.
	DocumentBar,
	/// Controls for adding, grouping, and deleting layers at the bottom of the Layers panel.
	LayersPanelBottomBar,
	/// Blending options at the top of the Layers panel.
	LayersPanelControlLeftBar,
	/// Selected layer status (locked/hidden) at the top of the Layers panel.
	LayersPanelControlRightBar,
	/// The dropdown menu at the very top of the application: File, Edit, etc.
	MenuBar,
	/// Bar at the top of the node graph containing the location and the "Preview" and "Hide" buttons.
	NodeGraphControlBar,
	/// The body of the Properties panel containing many collapsable sections.
	PropertiesPanel,
	/// The contextual input key/mouse combination shortcuts shown in the status bar at the bottom of the window.
	StatusBarHints,
	/// The left side of the control bar directly above the canvas.
	ToolOptions,
	/// The vertical buttons for all of the tools on the left of the canvas.
	ToolShelf,
	/// The quick access buttons found on the welcome screen, shown when no documents are open.
	WelcomeScreenButtons,
	/// The color swatch for the working colors and a flip and reset button found at the bottom of the tool shelf.
	WorkingColors,

	// KEEP THIS ENUM LAST
	// This is a marker that is used to define an array that is used to hold widgets
	_LayoutTargetLength,
}

/// For use by structs that define a UI widget layout by implementing the layout() function belonging to this trait.
/// The send_layout() function can then be called by other code which is a part of the same struct so as to send the layout to the frontend.
pub trait LayoutHolder {
	fn layout(&self) -> Layout;

	fn send_layout(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout { layout: self.layout(), layout_target });
	}
}

/// Structs implementing this hold the layout (like [`LayoutHolder`]) for dialog content, but it also requires defining the dialog's title, icon, and action buttons.
pub trait DialogLayoutHolder: LayoutHolder {
	const ICON: &'static str;
	const TITLE: &'static str;

	fn layout_buttons(&self) -> Layout;
	fn send_layout_buttons(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout_buttons(),
			layout_target,
		});
	}

	fn layout_column_2(&self) -> Layout {
		Layout::default()
	}
	fn send_layout_column_2(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout_column_2(),
			layout_target,
		});
	}

	fn send_dialog_to_frontend(&self, responses: &mut VecDeque<Message>) {
		self.send_layout(responses, LayoutTarget::DialogColumn1);
		self.send_layout_column_2(responses, LayoutTarget::DialogColumn2);
		self.send_layout_buttons(responses, LayoutTarget::DialogButtons);
		responses.add(FrontendMessage::DisplayDialog {
			icon: Self::ICON.into(),
			title: Self::TITLE.into(),
		});
	}
}

/// Trait for types that can compute incremental diffs for UI updates.
///
/// This trait unifies the diffing behavior across Layout, LayoutGroup, and WidgetInstance,
/// allowing each type to specify how it should be represented in a DiffUpdate.
pub trait Diffable: Clone + PartialEq {
	/// Converts this value into a DiffUpdate variant.
	fn into_diff_update(self) -> DiffUpdate;

	/// Computes the diff between self (old) and new, updating self and recording changes.
	fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>);

	/// Collects all CheckboxIds currently in use in this layout, computing stable replacements.
	fn collect_checkbox_ids(&self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &mut HashMap<CheckboxId, CheckboxId>);

	/// Replaces all widget IDs with deterministic IDs based on position and type.
	/// Also replaces CheckboxIds using the provided mapping.
	fn replace_widget_ids(&mut self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &HashMap<CheckboxId, CheckboxId>);
}

/// Computes a deterministic WidgetId based on layout target, path, and widget type.
fn compute_widget_id(layout_target: LayoutTarget, widget_path: &[usize], widget: &Widget) -> WidgetId {
	let mut hasher = DefaultHasher::new();

	(layout_target as u8).hash(&mut hasher);
	widget_path.hash(&mut hasher);
	std::mem::discriminant(widget).hash(&mut hasher);

	WidgetId(hasher.finish())
}

/// Computes a deterministic CheckboxId based on the same WidgetId algorithm.
fn compute_checkbox_id(layout_target: LayoutTarget, widget_path: &[usize], widget: &Widget) -> CheckboxId {
	let mut hasher = DefaultHasher::new();

	(layout_target as u8).hash(&mut hasher);
	widget_path.hash(&mut hasher);
	std::mem::discriminant(widget).hash(&mut hasher);

	// Add extra salt for checkbox to differentiate from widget ID
	"checkbox".hash(&mut hasher);

	CheckboxId(hasher.finish())
}

/// Contains an arrangement of widgets mounted somewhere specific in the frontend.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
pub struct Layout(pub Vec<LayoutGroup>);

impl Layout {
	pub fn iter(&self) -> WidgetIter<'_> {
		WidgetIter {
			stack: self.0.iter().collect(),
			..Default::default()
		}
	}

	pub fn iter_mut(&mut self) -> WidgetIterMut<'_> {
		WidgetIterMut {
			stack: self.0.iter_mut().collect(),
			..Default::default()
		}
	}
}

impl Diffable for Layout {
	fn into_diff_update(self) -> DiffUpdate {
		DiffUpdate::Layout(self)
	}

	fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		// Check if the length of items is different
		// TODO: Diff insersion and deletion of items
		if self.0.len() != new.0.len() {
			// Update the layout to the new layout
			self.0.clone_from(&new.0);

			// Push an update sublayout to the diff
			widget_diffs.push(WidgetDiff {
				widget_path: widget_path.to_vec(),
				new_value: new.into_diff_update(),
			});
			return;
		}
		// Diff all of the children
		for (index, (current_child, new_child)) in self.0.iter_mut().zip(new.0).enumerate() {
			widget_path.push(index);
			current_child.diff(new_child, widget_path, widget_diffs);
			widget_path.pop();
		}
	}

	fn collect_checkbox_ids(&self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &mut HashMap<CheckboxId, CheckboxId>) {
		for (index, child) in self.0.iter().enumerate() {
			widget_path.push(index);
			child.collect_checkbox_ids(layout_target, widget_path, checkbox_map);
			widget_path.pop();
		}
	}

	fn replace_widget_ids(&mut self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &HashMap<CheckboxId, CheckboxId>) {
		for (index, child) in self.0.iter_mut().enumerate() {
			widget_path.push(index);
			child.replace_widget_ids(layout_target, widget_path, checkbox_map);
			widget_path.pop();
		}
	}
}

#[derive(Debug, Default)]
pub struct WidgetIter<'a> {
	pub stack: Vec<&'a LayoutGroup>,
	pub table: Vec<&'a WidgetInstance>,
	pub current_slice: Option<&'a [WidgetInstance]>,
}

impl<'a> Iterator for WidgetIter<'a> {
	type Item = &'a WidgetInstance;

	fn next(&mut self) -> Option<Self::Item> {
		let widget = self.table.pop().or_else(|| {
			let (first, rest) = self.current_slice.take()?.split_first()?;
			self.current_slice = Some(rest);
			Some(first)
		});

		if let Some(item) = widget {
			if let WidgetInstance { widget: Widget::PopoverButton(p), .. } = item {
				self.stack.extend(p.popover_layout.0.iter());
				return self.next();
			}

			return Some(item);
		}

		match self.stack.pop() {
			Some(LayoutGroup::Column { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Row { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Table { rows, .. }) => {
				self.table.extend(rows.iter().flatten().rev());
				self.next()
			}
			Some(LayoutGroup::Section { layout, .. }) => {
				for layout_row in &layout.0 {
					self.stack.push(layout_row);
				}
				self.next()
			}
			None => None,
		}
	}
}

#[derive(Debug, Default)]
pub struct WidgetIterMut<'a> {
	pub stack: Vec<&'a mut LayoutGroup>,
	pub table: Vec<&'a mut WidgetInstance>,
	pub current_slice: Option<&'a mut [WidgetInstance]>,
}

impl<'a> Iterator for WidgetIterMut<'a> {
	type Item = &'a mut WidgetInstance;

	fn next(&mut self) -> Option<Self::Item> {
		let widget = self.table.pop().or_else(|| {
			let (first, rest) = self.current_slice.take()?.split_first_mut()?;
			self.current_slice = Some(rest);
			Some(first)
		});

		if let Some(widget) = widget {
			if let WidgetInstance { widget: Widget::PopoverButton(p), .. } = widget {
				self.stack.extend(p.popover_layout.0.iter_mut());
				return self.next();
			}

			return Some(widget);
		}

		match self.stack.pop() {
			Some(LayoutGroup::Column { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Row { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Table { rows, .. }) => {
				self.table.extend(rows.iter_mut().flatten().rev());
				self.next()
			}
			Some(LayoutGroup::Section { layout, .. }) => {
				for layout_row in &mut layout.0 {
					self.stack.push(layout_row);
				}
				self.next()
			}
			None => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum LayoutGroup {
	#[serde(rename = "column")]
	Column {
		#[serde(rename = "columnWidgets")]
		widgets: Vec<WidgetInstance>,
	},
	#[serde(rename = "row")]
	Row {
		#[serde(rename = "rowWidgets")]
		widgets: Vec<WidgetInstance>,
	},
	#[serde(rename = "table")]
	Table {
		#[serde(rename = "tableWidgets")]
		rows: Vec<Vec<WidgetInstance>>,
		unstyled: bool,
	},
	#[serde(rename = "section")]
	Section {
		name: String,
		description: String,
		visible: bool,
		pinned: bool,
		id: u64,
		layout: Layout,
	},
}

impl Default for LayoutGroup {
	fn default() -> Self {
		Self::Row { widgets: Vec::new() }
	}
}
impl From<Vec<WidgetInstance>> for LayoutGroup {
	fn from(widgets: Vec<WidgetInstance>) -> LayoutGroup {
		LayoutGroup::Row { widgets }
	}
}

impl LayoutGroup {
	/// Applies a tooltip label to all widgets in this row or column without a tooltip.
	pub fn with_tooltip_label(self, label: impl Into<String>) -> Self {
		let (is_col, mut widgets) = match self {
			LayoutGroup::Column { widgets } => (true, widgets),
			LayoutGroup::Row { widgets } => (false, widgets),
			_ => unimplemented!(),
		};
		let label = label.into();
		for widget in &mut widgets {
			let val = match &mut widget.widget {
				Widget::CheckboxInput(x) => &mut x.tooltip_label,
				Widget::ColorInput(x) => &mut x.tooltip_label,
				Widget::CurveInput(x) => &mut x.tooltip_label,
				Widget::DropdownInput(x) => &mut x.tooltip_label,
				Widget::FontInput(x) => &mut x.tooltip_label,
				Widget::IconButton(x) => &mut x.tooltip_label,
				Widget::IconLabel(x) => &mut x.tooltip_label,
				Widget::ImageButton(x) => &mut x.tooltip_label,
				Widget::ImageLabel(x) => &mut x.tooltip_label,
				Widget::NumberInput(x) => &mut x.tooltip_label,
				Widget::ParameterExposeButton(x) => &mut x.tooltip_label,
				Widget::PopoverButton(x) => &mut x.tooltip_label,
				Widget::TextAreaInput(x) => &mut x.tooltip_label,
				Widget::TextButton(x) => &mut x.tooltip_label,
				Widget::TextInput(x) => &mut x.tooltip_label,
				Widget::TextLabel(x) => &mut x.tooltip_label,
				Widget::BreadcrumbTrailButtons(x) => &mut x.tooltip_label,
				Widget::ReferencePointInput(_) | Widget::RadioInput(_) | Widget::Separator(_) | Widget::ShortcutLabel(_) | Widget::WorkingColorsInput(_) | Widget::NodeCatalog(_) => continue,
			};
			if val.is_empty() {
				val.clone_from(&label);
			}
		}
		if is_col { Self::Column { widgets } } else { Self::Row { widgets } }
	}

	/// Applies a tooltip description to all widgets in this row or column without a tooltip.
	pub fn with_tooltip_description(self, description: impl Into<String>) -> Self {
		let (is_col, mut widgets) = match self {
			LayoutGroup::Column { widgets } => (true, widgets),
			LayoutGroup::Row { widgets } => (false, widgets),
			_ => unimplemented!(),
		};
		let description = description.into();
		for widget in &mut widgets {
			let val = match &mut widget.widget {
				Widget::CheckboxInput(x) => &mut x.tooltip_description,
				Widget::ColorInput(x) => &mut x.tooltip_description,
				Widget::CurveInput(x) => &mut x.tooltip_description,
				Widget::DropdownInput(x) => &mut x.tooltip_description,
				Widget::FontInput(x) => &mut x.tooltip_description,
				Widget::IconButton(x) => &mut x.tooltip_description,
				Widget::IconLabel(x) => &mut x.tooltip_description,
				Widget::ImageButton(x) => &mut x.tooltip_description,
				Widget::ImageLabel(x) => &mut x.tooltip_description,
				Widget::NumberInput(x) => &mut x.tooltip_description,
				Widget::ParameterExposeButton(x) => &mut x.tooltip_description,
				Widget::PopoverButton(x) => &mut x.tooltip_description,
				Widget::TextAreaInput(x) => &mut x.tooltip_description,
				Widget::TextButton(x) => &mut x.tooltip_description,
				Widget::TextInput(x) => &mut x.tooltip_description,
				Widget::TextLabel(x) => &mut x.tooltip_description,
				Widget::BreadcrumbTrailButtons(x) => &mut x.tooltip_description,
				Widget::ReferencePointInput(_) | Widget::RadioInput(_) | Widget::Separator(_) | Widget::ShortcutLabel(_) | Widget::WorkingColorsInput(_) | Widget::NodeCatalog(_) => continue,
			};
			if val.is_empty() {
				val.clone_from(&description);
			}
		}
		if is_col { Self::Column { widgets } } else { Self::Row { widgets } }
	}

	pub fn iter_mut(&mut self) -> WidgetIterMut<'_> {
		WidgetIterMut {
			stack: vec![self],
			..Default::default()
		}
	}
}

impl Diffable for LayoutGroup {
	fn into_diff_update(self) -> DiffUpdate {
		DiffUpdate::LayoutGroup(self)
	}

	fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		let is_column = matches!(new, Self::Column { .. });
		match (self, new) {
			(Self::Column { widgets: current_widgets }, Self::Column { widgets: new_widgets }) | (Self::Row { widgets: current_widgets }, Self::Row { widgets: new_widgets }) => {
				// If the lengths are different then resend the entire panel
				// TODO: Diff insersion and deletion of items
				if current_widgets.len() != new_widgets.len() {
					// Update to the new value
					current_widgets.clone_from(&new_widgets);

					// Push back a LayoutGroup update to the diff
					let new_value = (if is_column { Self::Column { widgets: new_widgets } } else { Self::Row { widgets: new_widgets } }).into_diff_update();
					let widget_path = widget_path.to_vec();
					widget_diffs.push(WidgetDiff { widget_path, new_value });
					return;
				}
				// Diff all of the children
				for (index, (current_child, new_child)) in current_widgets.iter_mut().zip(new_widgets).enumerate() {
					widget_path.push(index);
					current_child.diff(new_child, widget_path, widget_diffs);
					widget_path.pop();
				}
			}
			(
				Self::Section {
					name: current_name,
					description: current_description,
					visible: current_visible,
					pinned: current_pinned,
					id: current_id,
					layout: current_layout,
				},
				Self::Section {
					name: new_name,
					description: new_description,
					visible: new_visible,
					pinned: new_pinned,
					id: new_id,
					layout: new_layout,
				},
			) => {
				// Resend the entire panel if the lengths, names, visibility, or node IDs are different
				// TODO: Diff insersion and deletion of items
				if current_layout.0.len() != new_layout.0.len()
					|| *current_name != new_name
					|| *current_description != new_description
					|| *current_visible != new_visible
					|| *current_pinned != new_pinned
					|| *current_id != new_id
				{
					// Update self to reflect new changes
					current_name.clone_from(&new_name);
					current_description.clone_from(&new_description);
					*current_visible = new_visible;
					*current_pinned = new_pinned;
					*current_id = new_id;
					current_layout.clone_from(&new_layout);

					// Push an update layout group to the diff
					let new_value = Self::Section {
						name: new_name,
						description: new_description,
						visible: new_visible,
						pinned: new_pinned,
						id: new_id,
						layout: new_layout,
					}
					.into_diff_update();
					let widget_path = widget_path.to_vec();
					widget_diffs.push(WidgetDiff { widget_path, new_value });
				}
				// Diff all of the children
				else {
					for (index, (current_child, new_child)) in current_layout.0.iter_mut().zip(new_layout.0).enumerate() {
						widget_path.push(index);
						current_child.diff(new_child, widget_path, widget_diffs);
						widget_path.pop();
					}
				}
			}
			(current, new) => {
				*current = new.clone();
				let new_value = new.into_diff_update();
				let widget_path = widget_path.to_vec();
				widget_diffs.push(WidgetDiff { widget_path, new_value });
			}
		}
	}

	fn collect_checkbox_ids(&self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &mut HashMap<CheckboxId, CheckboxId>) {
		match self {
			Self::Column { widgets } | Self::Row { widgets } => {
				for (index, widget) in widgets.iter().enumerate() {
					widget_path.push(index);
					widget.collect_checkbox_ids(layout_target, widget_path, checkbox_map);
					widget_path.pop();
				}
			}
			Self::Table { rows, .. } => {
				for (row_idx, row) in rows.iter().enumerate() {
					for (col_idx, widget) in row.iter().enumerate() {
						widget_path.push(row_idx);
						widget_path.push(col_idx);
						widget.collect_checkbox_ids(layout_target, widget_path, checkbox_map);
						widget_path.pop();
						widget_path.pop();
					}
				}
			}
			Self::Section { layout, .. } => {
				layout.collect_checkbox_ids(layout_target, widget_path, checkbox_map);
			}
		}
	}

	fn replace_widget_ids(&mut self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &HashMap<CheckboxId, CheckboxId>) {
		match self {
			Self::Column { widgets } | Self::Row { widgets } => {
				for (index, widget) in widgets.iter_mut().enumerate() {
					widget_path.push(index);
					widget.replace_widget_ids(layout_target, widget_path, checkbox_map);
					widget_path.pop();
				}
			}
			Self::Table { rows, .. } => {
				for (row_idx, row) in rows.iter_mut().enumerate() {
					for (col_idx, widget) in row.iter_mut().enumerate() {
						widget_path.push(row_idx);
						widget_path.push(col_idx);
						widget.replace_widget_ids(layout_target, widget_path, checkbox_map);
						widget_path.pop();
						widget_path.pop();
					}
				}
			}
			Self::Section { layout, .. } => {
				layout.replace_widget_ids(layout_target, widget_path, checkbox_map);
			}
		}
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WidgetInstance {
	#[serde(rename = "widgetId")]
	pub widget_id: WidgetId,
	pub widget: Widget,
}

impl PartialEq for WidgetInstance {
	fn eq(&self, other: &Self) -> bool {
		self.widget_id == other.widget_id && self.widget == other.widget
	}
}

impl WidgetInstance {
	#[deprecated(since = "0.0.0", note = "Please use the builder pattern, e.g. TextLabel::new(\"hello\").widget_instance()")]
	pub fn new(widget: Widget) -> Self {
		Self {
			widget_id: WidgetId(generate_uuid()),
			widget,
		}
	}
}

impl Diffable for WidgetInstance {
	fn into_diff_update(self) -> DiffUpdate {
		DiffUpdate::Widget(self)
	}

	fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		if self == &new {
			// Still need to update callbacks since PartialEq skips them
			self.widget = new.widget;
			return;
		}

		// Special handling for PopoverButton: recursively diff nested layout if only the layout changed
		if let (Widget::PopoverButton(button1), Widget::PopoverButton(button2)) = (&mut self.widget, &new.widget) {
			// Check if only the popover layout changed (all other fields are the same)
			if self.widget_id == new.widget_id
				&& button1.disabled == button2.disabled
				&& button1.style == button2.style
				&& button1.menu_direction == button2.menu_direction
				&& button1.icon == button2.icon
				&& button1.tooltip_label == button2.tooltip_label
				&& button1.tooltip_description == button2.tooltip_description
				&& button1.tooltip_shortcut == button2.tooltip_shortcut
				&& button1.popover_min_width == button2.popover_min_width
			{
				// Only the popover layout differs, diff it recursively
				for (i, (a, b)) in button1.popover_layout.0.iter_mut().zip(button2.popover_layout.0.iter()).enumerate() {
					widget_path.push(i);
					a.diff(b.clone(), widget_path, widget_diffs);
					widget_path.pop();
				}
				return;
			}
		}

		// Widget or ID changed, send full update
		*self = new.clone();
		let new_value = new.into_diff_update();
		let widget_path = widget_path.to_vec();
		widget_diffs.push(WidgetDiff { widget_path, new_value });
	}

	fn collect_checkbox_ids(&self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &mut HashMap<CheckboxId, CheckboxId>) {
		match &self.widget {
			Widget::CheckboxInput(checkbox) => {
				// Compute stable ID based on position and insert mapping
				let checkbox_id = checkbox.for_label;
				let stable_id = compute_checkbox_id(layout_target, widget_path, &self.widget);
				checkbox_map.entry(checkbox_id).or_insert(stable_id);
			}
			Widget::TextLabel(label) => {
				// Compute stable ID based on position and insert mapping
				let checkbox_id = label.for_checkbox;
				let stable_id = compute_checkbox_id(layout_target, widget_path, &self.widget);
				checkbox_map.entry(checkbox_id).or_insert(stable_id);
			}
			Widget::PopoverButton(button) => {
				// Recursively collect from nested popover layout
				for (index, child) in button.popover_layout.0.iter().enumerate() {
					widget_path.push(index);
					child.collect_checkbox_ids(layout_target, widget_path, checkbox_map);
					widget_path.pop();
				}
			}
			_ => {}
		}
	}

	fn replace_widget_ids(&mut self, layout_target: LayoutTarget, widget_path: &mut Vec<usize>, checkbox_map: &HashMap<CheckboxId, CheckboxId>) {
		// 1. Generate deterministic WidgetId
		self.widget_id = compute_widget_id(layout_target, widget_path, &self.widget);

		// 2. Replace CheckboxIds if present
		match &mut self.widget {
			Widget::CheckboxInput(checkbox) => {
				let old_id = checkbox.for_label;
				if let Some(&new_id) = checkbox_map.get(&old_id) {
					checkbox.for_label = new_id;
				}
			}
			Widget::TextLabel(label) => {
				let old_id = label.for_checkbox;
				if let Some(&new_id) = checkbox_map.get(&old_id) {
					label.for_checkbox = new_id;
				}
			}
			Widget::PopoverButton(button) => {
				// Recursively replace in nested popover layout
				for (index, child) in button.popover_layout.0.iter_mut().enumerate() {
					widget_path.push(index);
					child.replace_widget_ids(layout_target, widget_path, checkbox_map);
					widget_path.pop();
				}
			}
			_ => {}
		}
	}
}

#[derive(Clone, specta::Type)]
pub struct WidgetCallback<T> {
	#[specta(skip)]
	pub callback: Arc<dyn Fn(&T) -> Message + 'static + Send + Sync>,
}

impl<T> WidgetCallback<T> {
	pub fn new(callback: impl Fn(&T) -> Message + 'static + Send + Sync) -> Self {
		Self { callback: Arc::new(callback) }
	}
}

impl<T> Default for WidgetCallback<T> {
	fn default() -> Self {
		Self::new(|_| Message::NoOp)
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Widget {
	BreadcrumbTrailButtons(BreadcrumbTrailButtons),
	CheckboxInput(CheckboxInput),
	ColorInput(ColorInput),
	CurveInput(CurveInput),
	DropdownInput(DropdownInput),
	FontInput(FontInput),
	IconButton(IconButton),
	IconLabel(IconLabel),
	ImageButton(ImageButton),
	ImageLabel(ImageLabel),
	ShortcutLabel(ShortcutLabel),
	NodeCatalog(NodeCatalog),
	NumberInput(NumberInput),
	ParameterExposeButton(ParameterExposeButton),
	ReferencePointInput(ReferencePointInput),
	PopoverButton(PopoverButton),
	RadioInput(RadioInput),
	Separator(Separator),
	TextAreaInput(TextAreaInput),
	TextButton(TextButton),
	TextInput(TextInput),
	TextLabel(TextLabel),
	WorkingColorsInput(WorkingColorsInput),
}

/// A single change to part of the UI, containing the location of the change and the new value.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WidgetDiff {
	/// A path to the change
	/// e.g. [0, 1, 2] in the properties panel is the first section, second row and third widget.
	/// An empty path [] shows that the entire panel has changed and is sent when the UI is first created.
	#[serde(rename = "widgetPath")]
	pub widget_path: Vec<usize>,
	/// What the specified part of the UI has changed to.
	#[serde(rename = "newValue")]
	pub new_value: DiffUpdate,
}

/// The new value of the UI, sent as part of a diff.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum DiffUpdate {
	#[serde(rename = "layout")]
	Layout(Layout),
	#[serde(rename = "layoutGroup")]
	LayoutGroup(LayoutGroup),
	#[serde(rename = "widget")]
	Widget(WidgetInstance),
}

impl DiffUpdate {
	/// Append the keyboard shortcut to the tooltip where applicable
	pub fn apply_keyboard_shortcut(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Option<KeysGroup>) {
		// Go through each widget to convert `ActionShortcut::Action` to `ActionShortcut::Shortcut` and append the key combination to the widget tooltip
		let convert_tooltip = |widget_instance: &mut WidgetInstance| {
			// Handle all the widgets that have tooltips
			let tooltip_shortcut = match &mut widget_instance.widget {
				Widget::BreadcrumbTrailButtons(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::CheckboxInput(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::ColorInput(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::DropdownInput(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::FontInput(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::IconButton(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::NumberInput(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::ParameterExposeButton(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::PopoverButton(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::TextButton(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::ImageButton(widget) => widget.tooltip_shortcut.as_mut(),
				Widget::ShortcutLabel(widget) => widget.shortcut.as_mut(),
				Widget::IconLabel(_)
				| Widget::ImageLabel(_)
				| Widget::CurveInput(_)
				| Widget::NodeCatalog(_)
				| Widget::ReferencePointInput(_)
				| Widget::RadioInput(_)
				| Widget::Separator(_)
				| Widget::TextAreaInput(_)
				| Widget::TextInput(_)
				| Widget::TextLabel(_)
				| Widget::WorkingColorsInput(_) => None,
			};

			// Convert `ActionShortcut::Action` to `ActionShortcut::Shortcut`
			if let Some(tooltip_shortcut) = tooltip_shortcut {
				tooltip_shortcut.realize_shortcut(action_input_mapping);
			}

			// Handle RadioInput separately because its tooltips are children of the widget
			if let Widget::RadioInput(radio_input) = &mut widget_instance.widget {
				for radio_entry_data in &mut radio_input.entries {
					// Convert `ActionShortcut::Action` to `ActionShortcut::Shortcut`
					if let Some(tooltip_shortcut) = radio_entry_data.tooltip_shortcut.as_mut() {
						tooltip_shortcut.realize_shortcut(action_input_mapping);
					}
				}
			}
		};

		// Recursively fill menu list entries with their realized shortcut keys specific to the current bindings and platform
		let apply_action_shortcut_to_menu_lists = |entry_sections: &mut MenuListEntrySections| {
			struct RecursiveWrapper<'a>(&'a dyn Fn(&mut MenuListEntrySections, &RecursiveWrapper));
			let recursive_wrapper = RecursiveWrapper(&|entry_sections: &mut MenuListEntrySections, recursive_wrapper| {
				for entries in entry_sections {
					for entry in entries {
						// Convert `ActionShortcut::Action` to `ActionShortcut::Shortcut`
						if let Some(tooltip_shortcut) = &mut entry.tooltip_shortcut {
							tooltip_shortcut.realize_shortcut(action_input_mapping);
						}

						// Recursively call this inner closure on the menu's children
						(recursive_wrapper.0)(&mut entry.children, recursive_wrapper);
					}
				}
			});
			(recursive_wrapper.0)(entry_sections, &recursive_wrapper)
		};

		// Apply shortcut conversions to all widgets that have menu lists
		let convert_menu_lists = |widget_instance: &mut WidgetInstance| match &mut widget_instance.widget {
			Widget::DropdownInput(dropdown_input) => apply_action_shortcut_to_menu_lists(&mut dropdown_input.entries),
			Widget::TextButton(text_button) => apply_action_shortcut_to_menu_lists(&mut text_button.menu_list_children),
			_ => {}
		};

		match self {
			Self::Layout(layout) => layout.0.iter_mut().flat_map(|layout_group| layout_group.iter_mut()).for_each(|widget_instance| {
				convert_tooltip(widget_instance);
				convert_menu_lists(widget_instance);
			}),
			Self::LayoutGroup(layout_group) => layout_group.iter_mut().for_each(|widget_instance| {
				convert_tooltip(widget_instance);
				convert_menu_lists(widget_instance);
			}),
			Self::Widget(widget_instance) => {
				convert_tooltip(widget_instance);
				convert_menu_lists(widget_instance);
			}
		}
	}
}
