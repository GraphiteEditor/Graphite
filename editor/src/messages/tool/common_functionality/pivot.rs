//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale.

use crate::consts::PIVOT_DIAMETER;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::path_tool::PathOptionsUpdate;
use crate::messages::tool::tool_messages::select_tool::SelectOptionsUpdate;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::{transform::ReferencePoint, vector::ManipulatorPointId};
use std::fmt;

pub fn pin_pivot_widget(inactive: bool, enabled: bool, source: Source) -> WidgetHolder {
	IconButton::new(if inactive { "PinInactive" } else { "PinActive" }, 24)
		.tooltip(if inactive { "Pin Transform Pivot" } else { "Unpin Transform Pivot" })
		.disabled(!enabled)
		.on_update(move |_| match source {
			Source::Select if enabled => SelectToolMessage::SelectOptions(SelectOptionsUpdate::TogglePivotPinned()).into(),
			Source::Path if enabled => PathToolMessage::UpdateOptions(PathOptionsUpdate::TogglePivotPinned()).into(),
			_ => Message::NoOp,
		})
		.widget_holder()
}

pub fn pivot_reference_point_widget(disabled: bool, reference_point: ReferencePoint, source: Source) -> WidgetHolder {
	ReferencePointInput::new(reference_point)
		.on_update(move |pivot_input: &ReferencePointInput| match source {
			Source::Select => SelectToolMessage::SetPivot { position: pivot_input.value }.into(),
			Source::Path => PathToolMessage::SetPivot { position: pivot_input.value }.into(),
		})
		.disabled(disabled)
		.widget_holder()
}

pub fn dot_type_widget(state: DotState, source: Source) -> Vec<WidgetHolder> {
	let dot_type_entries = [DotType::Pivot, DotType::Average, DotType::Active]
		.iter()
		.map(|dot_type| {
			MenuListEntry::new(format!("{dot_type:?}")).label(dot_type.to_string()).on_commit({
				let value = source.clone();
				move |_| match value {
					Source::Select => SelectToolMessage::SelectOptions(SelectOptionsUpdate::DotType(*dot_type)).into(),
					Source::Path => PathToolMessage::UpdateOptions(PathOptionsUpdate::DotType(*dot_type)).into(),
				}
			})
		})
		.collect();

	vec![
		CheckboxInput::new(state.enabled)
			.tooltip("Disable Transform Pivot Point")
			.on_update(move |optional_input: &CheckboxInput| match source {
				Source::Select => SelectToolMessage::SelectOptions(SelectOptionsUpdate::ToggleDotType(optional_input.checked)).into(),
				Source::Path => PathToolMessage::UpdateOptions(PathOptionsUpdate::ToggleDotType(optional_input.checked)).into(),
			})
			.widget_holder(),
		Separator::new(SeparatorType::Related).widget_holder(),
		DropdownInput::new(vec![dot_type_entries])
			.selected_index(Some(match state.dot {
				DotType::Pivot => 0,
				DotType::Average => 1,
				DotType::Active => 2,
			}))
			.tooltip("Choose between type of Transform Pivot Point")
			.disabled(!state.enabled)
			.widget_holder(),
	]
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Source {
	Path,
	#[default]
	Select,
}

#[derive(PartialEq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Dot {
	pub pivot: Pivot,
	pub state: DotState,
	pub layer: Option<LayerNodeIdentifier>,
	pub point: Option<ManipulatorPointId>,
}

impl Dot {
	pub fn position(&self, document: &DocumentMessageHandler) -> DVec2 {
		let network = &document.network_interface;
		self.state
			.enabled
			.then_some({
				match self.state.dot {
					DotType::Average => Some(network.selected_nodes().selected_visible_and_unlocked_layers_mean_average_origin(network)),
					DotType::Pivot => self.pivot.position(),
					DotType::Active => self.layer.map(|layer| graph_modification_utils::get_viewport_origin(layer, network)),
				}
			})
			.flatten()
			.unwrap_or_else(|| self.pivot.transform_from_normalized.transform_point2(DVec2::splat(0.5)))
	}

	pub fn recalculate_transform(&mut self, document: &DocumentMessageHandler) -> DAffine2 {
		self.pivot.recalculate_pivot(document);
		self.pivot.transform_from_normalized
	}

	pub fn pin_inactive(&self) -> bool {
		!self.pivot.pinned || !self.state.is_pivot()
	}
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum DotType {
	// Pivot
	#[default]
	Pivot,
	// Origin
	Average,
	Active,
	// TODO: Add "Individual"
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DotState {
	pub enabled: bool,
	pub dot: DotType,
}

impl Default for DotState {
	fn default() -> Self {
		Self {
			enabled: true,
			dot: DotType::default(),
		}
	}
}

impl DotState {
	pub fn is_pivot_type(&self) -> bool {
		self.dot == DotType::Pivot || !self.enabled
	}

	pub fn is_pivot(&self) -> bool {
		self.dot == DotType::Pivot && self.enabled
	}
}

impl fmt::Display for DotType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			DotType::Pivot => write!(f, "Custom Pivot"),
			DotType::Average => write!(f, "Origin (Average Point)"),
			DotType::Active => write!(f, "Origin (Active Object)"),
			// TODO: Add "Origin (Individual)"
		}
	}
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Pivot {
	/// Pivot between (0,0) and (1,1)
	normalized_pivot: DVec2,
	/// Transform to get from normalized pivot to viewspace
	pub transform_from_normalized: DAffine2,
	/// The viewspace pivot position
	pivot: Option<DVec2>,
	/// The old pivot position in the GUI, used to reduce refreshes of the document bar
	pub old_pivot_position: ReferencePoint,
	/// The last ReferencePoint which wasn't none
	pub last_non_none_reference: ReferencePoint,
	/// Used to enable and disable the pivot
	active: bool,
	/// Used to enable and disable the pivot
	pub pinned: bool,
	/// Had selected_visible_and_unlocked_layers
	pub empty: bool,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			normalized_pivot: DVec2::splat(0.5),
			transform_from_normalized: Default::default(),
			pivot: Default::default(),
			old_pivot_position: ReferencePoint::Center,
			last_non_none_reference: ReferencePoint::Center,
			active: true,
			pinned: false,
			empty: true,
		}
	}
}

impl Pivot {
	/// Recomputes the pivot position and transform.
	pub fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		if !self.active {
			return;
		}

		let selected = document.network_interface.selected_nodes();

		self.empty = !selected.has_selected_nodes();
		if !selected.has_selected_nodes() {
			return;
		};

		let transform = selected
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
			.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
			.unwrap_or_default();

		let bounds = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
			.filter_map(|layer| {
				document
					.metadata()
					.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
			})
			.reduce(graphene_std::renderer::Quad::combine_bounds);

		let [min, max] = bounds.unwrap_or([DVec2::ZERO, DVec2::ONE]);
		self.transform_from_normalized = transform * DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		if self.old_pivot_position != ReferencePoint::None {
			self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
		}
	}

	pub fn recalculate_pivot_for_layer(&mut self, document: &DocumentMessageHandler, bounds: Option<[DVec2; 2]>) {
		if !self.active {
			return;
		}

		let selected = document.network_interface.selected_nodes();
		if !selected.has_selected_nodes() {
			self.normalized_pivot = DVec2::splat(0.5);
			self.pivot = None;
			return;
		};

		let [min, max] = bounds.unwrap_or([DVec2::ZERO, DVec2::ONE]);
		self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, draw_data: Option<(f64,)>, draw: bool) {
		if !overlay_context.visibility_settings.pivot() {
			self.active = false;
			return;
		} else {
			self.active = true;
		}

		self.recalculate_pivot(document);
		if !draw {
			return;
		};
		if let (Some(pivot), Some(data)) = (self.pivot, draw_data) {
			overlay_context.pivot(pivot, data.0);
		}
	}

	/// Answers if the pivot widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_pivot_position(&mut self) -> bool {
		if !self.active {
			return false;
		}

		let new = self.to_pivot_position();
		let should_refresh = new != self.old_pivot_position;
		self.old_pivot_position = new;
		should_refresh
	}

	pub fn to_pivot_position(&self) -> ReferencePoint {
		self.normalized_pivot.into()
	}

	pub fn position(&self) -> Option<DVec2> {
		self.pivot
	}

	/// Sets the viewport position of the pivot.
	pub fn set_viewport_position(&mut self, position: DVec2) {
		if !self.active {
			return;
		}

		if self.transform_from_normalized.matrix2.determinant().abs() <= f64::EPSILON {
			return;
		};

		self.normalized_pivot = self.transform_from_normalized.inverse().transform_point2(position);
		self.pivot = Some(position);
	}

	/// Set the pivot using a normalized position.
	pub fn set_normalized_position(&mut self, position: DVec2) {
		if !self.active {
			return;
		}
		self.normalized_pivot = position;
		self.pivot = Some(self.transform_from_normalized.transform_point2(position));
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		if !self.active {
			return false;
		}
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_DIAMETER / 2.).powi(2)).is_some()
	}
}
