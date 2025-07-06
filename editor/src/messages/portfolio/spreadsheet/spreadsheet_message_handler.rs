use super::VectorDataDomain;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget, WidgetLayout};
use crate::messages::portfolio::spreadsheet::InspectInputConnector;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::{AbsoluteInputConnector, NodeId};
use graphene_std::Color;
use graphene_std::Context;
use graphene_std::GraphicGroupTable;
use graphene_std::instances::Instances;
use graphene_std::memo::IORecord;
use graphene_std::raster::Image;
use graphene_std::uuid::CompiledProtonodeInput;
use graphene_std::vector::{VectorData, VectorDataTable};
use graphene_std::{Artboard, ArtboardGroupTable, GraphicElement};
use std::any::Any;
use std::sync::Arc;

pub struct SpreadsheetMessageHandlerData {
	pub introspected_data: &HashMap<CompiledProtonodeInput, Box<dyn std::any::Any + Send + Sync>>;
}

/// The spreadsheet UI allows for instance data to be previewed.
#[derive(Default, Debug, Clone, ExtractField)]
pub struct SpreadsheetMessageHandler {
	/// Sets whether or not the spreadsheet is drawn.
	pub spreadsheet_view_open: bool,
	inspect_input: Option<InspectInputConnector>,
	// Downcasted data is not saved because the spreadsheet is simply a window into the data flowing through the input
	// introspected_data: Option<TaggedValue>,
	instances_path: Vec<usize>,
	viewing_vector_data_domain: VectorDataDomain,
}

#[message_handler_data]
impl MessageHandler<SpreadsheetMessage, SpreadsheetMessageHandlerData> for SpreadsheetMessageHandler {
	fn process_message(&mut self, message: SpreadsheetMessage, responses: &mut VecDeque<Message>, data: SpreadsheetMessageHandlerData) {
		let {introspected_data} = data;
		match message {
			SpreadsheetMessage::ToggleOpen => {
				self.spreadsheet_view_open = !self.spreadsheet_view_open;
				if self.spreadsheet_view_open {
					// TODO: This will not get always get data since the input could be cached, and the monitor node would not
					// Be run on the evaluation. To solve this, pass in an AbsoluteNodeInput as a parameter to the compilation which tells the compiler
					// to generate a random SNI in order to reset any downstream cache
					// Run the graph to grab the data
					responses.add(PortfolioMessage::EvaluateActiveDocument);
				}
				// Update checked UI state for open
				responses.add(MenuBarMessage::SendLayout);
				self.update_layout(responses);
			}

			// Queued on introspection request, runs on introspection response when the data has been sent back to the editor
			SpreadsheetMessage::UpdateLayout { inpect_input } => {
				self.inspect_input = Some(inpect_input);
				self.update_layout(introspected_data, responses);
			}

			SpreadsheetMessage::PushToInstancePath { index } => {
				self.instances_path.push(index);
				self.update_layout(introspected_data, responses);
			}
			SpreadsheetMessage::TruncateInstancePath { len } => {
				self.instances_path.truncate(len);
				self.update_layout(introspected_data, responses);
			}

			SpreadsheetMessage::ViewVectorDataDomain { domain } => {
				self.viewing_vector_data_domain = domain;
				self.update_layout(introspected_data, responses);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(SpreadsheetMessage;)
	}
}

impl SpreadsheetMessageHandler {
	fn update_layout(&mut self, introspected_data: &HashMap<CompiledProtonodeInput, Box<dyn std::any::Any + Send + Sync>>, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateSpreadsheetState {
			// The node is sent when the data is available
			node: None,
			open: self.spreadsheet_view_open,
		});
		if !self.spreadsheet_view_open {
			return;
		}
		let mut layout_data = LayoutData {
			current_depth: 0,
			desired_path: &mut self.instances_path,
			breadcrumbs: Vec::new(),
			vector_data_domain: self.viewing_vector_data_domain,
		};
		let mut layout = match self.inspect_input {
			Some(inspect_input) => {
				match introspected_data.get(&inspect_input.protonode_input){
					Some(data) => {
						match generate_layout(instrospected_data, &mut layout_data) {
							Some(layout) => layout,
							None => label("The introspected data is not a supported type to be displayed."),
						}
					},
					None => label("Introspected data is not available for this input. This input may be cached."),
				}
			},
			None => label("No input selected to show data for."),
		};

		if layout_data.breadcrumbs.len() > 1 {
			let breadcrumb = BreadcrumbTrailButtons::new(layout_data.breadcrumbs)
				.on_update(|&len| SpreadsheetMessage::TruncateInstancePath { len: len as usize }.into())
				.widget_holder();
			layout.insert(0, LayoutGroup::Row { widgets: vec![breadcrumb] });
		}

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout { layout }),
			layout_target: LayoutTarget::Spreadsheet,
		});
	}
}

struct LayoutData<'a> {
	current_depth: usize,
	desired_path: &'a mut Vec<usize>,
	breadcrumbs: Vec<String>,
	vector_data_domain: VectorDataDomain,
}

fn generate_layout(introspected_data: &Box<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// We simply try random types. TODO: better strategy.
	#[allow(clippy::manual_map)]
	if let Some(io) = introspected_data.downcast_ref::<ArtboardGroupTable>() {
		Some(io.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<VectorDataTable>() {
		Some(io.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<GraphicGroupTable>() {
		Some(io.layout_with_breadcrumb(data))
	} else {
		None
	}
}

fn column_headings(value: &[&str]) -> Vec<WidgetHolder> {
	value.iter().map(|text| TextLabel::new(*text).widget_holder()).collect()
}

fn label(x: impl Into<String>) -> Vec<LayoutGroup> {
	let error = vec![TextLabel::new(x).widget_holder()];
	vec![LayoutGroup::Row { widgets: error }]
}

trait InstanceLayout {
	fn type_name() -> &'static str;
	fn identifier(&self) -> String;
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(self.identifier());
		self.compute_layout(data)
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup>;
}

impl InstanceLayout for GraphicElement {
	fn type_name() -> &'static str {
		"GraphicElement"
	}
	fn identifier(&self) -> String {
		match self {
			Self::GraphicGroup(instances) => instances.identifier(),
			Self::VectorData(instances) => instances.identifier(),
			Self::RasterDataCPU(_) => "RasterDataCPU".to_string(),
			Self::RasterDataGPU(_) => "RasterDataGPU".to_string(),
		}
	}
	// Don't put a breadcrumb for GraphicElement
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.compute_layout(data)
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Self::GraphicGroup(instances) => instances.layout_with_breadcrumb(data),
			Self::VectorData(instances) => instances.layout_with_breadcrumb(data),
			Self::RasterDataCPU(_) => label("Raster frame not supported"),
			Self::RasterDataGPU(_) => label("Raster frame not supported"),
		}
	}
}

impl InstanceLayout for VectorData {
	fn type_name() -> &'static str {
		"VectorData"
	}
	fn identifier(&self) -> String {
		format!("Vector Data (points={}, segments={})", self.point_domain.ids().len(), self.segment_domain.ids().len())
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		let colinear = self.colinear_manipulators.iter().map(|[a, b]| format!("[{a} / {b}]")).collect::<Vec<_>>().join(", ");
		let colinear = if colinear.is_empty() { "None" } else { &colinear };
		let style = vec![
			TextLabel::new(format!(
				"{}\n\nColinear Handle IDs: {}\n\nUpstream Graphic Group Table: {}",
				self.style,
				colinear,
				if self.upstream_graphic_group.is_some() { "Yes" } else { "No" }
			))
			.multiline(true)
			.widget_holder(),
		];

		let domain_entries = [VectorDataDomain::Points, VectorDataDomain::Segments, VectorDataDomain::Regions]
			.into_iter()
			.map(|domain| {
				RadioEntryData::new(format!("{domain:?}"))
					.label(format!("{domain:?}"))
					.on_update(move |_| SpreadsheetMessage::ViewVectorDataDomain { domain }.into())
			})
			.collect();
		let domain = vec![RadioInput::new(domain_entries).selected_index(Some(data.vector_data_domain as u32)).widget_holder()];

		let mut table_rows = Vec::new();
		match data.vector_data_domain {
			VectorDataDomain::Points => {
				table_rows.push(column_headings(&["", "position"]));
				table_rows.extend(
					self.point_domain
						.iter()
						.map(|(id, position)| vec![TextLabel::new(format!("{}", id.inner())).widget_holder(), TextLabel::new(format!("{}", position)).widget_holder()]),
				);
			}
			VectorDataDomain::Segments => {
				table_rows.push(column_headings(&["", "start_index", "end_index", "handles"]));
				table_rows.extend(self.segment_domain.iter().map(|(id, start, end, handles)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).widget_holder(),
						TextLabel::new(format!("{}", start)).widget_holder(),
						TextLabel::new(format!("{}", end)).widget_holder(),
						TextLabel::new(format!("{:?}", handles)).widget_holder(),
					]
				}));
			}
			VectorDataDomain::Regions => {
				table_rows.push(column_headings(&["", "segment_range", "fill"]));
				table_rows.extend(self.region_domain.iter().map(|(id, segment_range, fill)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).widget_holder(),
						TextLabel::new(format!("{:?}", segment_range)).widget_holder(),
						TextLabel::new(format!("{}", fill.inner())).widget_holder(),
					]
				}));
			}
		}

		vec![LayoutGroup::Row { widgets: style }, LayoutGroup::Row { widgets: domain }, LayoutGroup::Table { rows: table_rows }]
	}
}

impl InstanceLayout for Image<Color> {
	fn type_name() -> &'static str {
		"Image"
	}
	fn identifier(&self) -> String {
		format!("Image (width={}, height={})", self.width, self.height)
	}
	fn compute_layout(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let rows = vec![vec![TextLabel::new(format!("Image (width={}, height={})", self.width, self.height)).widget_holder()]];
		vec![LayoutGroup::Table { rows }]
	}
}

impl InstanceLayout for Artboard {
	fn type_name() -> &'static str {
		"Artboard"
	}
	fn identifier(&self) -> String {
		self.label.clone()
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.graphic_group.compute_layout(data)
	}
}

impl<T: InstanceLayout> InstanceLayout for Instances<T> {
	fn type_name() -> &'static str {
		"Instances"
	}
	fn identifier(&self) -> String {
		format!("Instances<{}> (length={})", T::type_name(), self.len())
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		if let Some(index) = data.desired_path.get(data.current_depth).copied() {
			if let Some(instance) = self.get(index) {
				data.current_depth += 1;
				let result = instance.instance.layout_with_breadcrumb(data);
				data.current_depth -= 1;
				return result;
			} else {
				warn!("Desired path truncated");
				data.desired_path.truncate(data.current_depth);
			}
		}

		let mut rows = self
			.instance_ref_iter()
			.enumerate()
			.map(|(index, instance)| {
				let (scale, angle, translation) = instance.transform.to_scale_angle_translation();
				let rotation = if angle == -0. { 0. } else { angle.to_degrees() };
				let round = |x: f64| (x * 1e3).round() / 1e3;
				vec![
					TextLabel::new(format!("{}", index)).widget_holder(),
					TextButton::new(instance.instance.identifier())
						.on_update(move |_| SpreadsheetMessage::PushToInstancePath { index }.into())
						.widget_holder(),
					TextLabel::new(format!(
						"Location: ({} px, {} px) — Rotation: {rotation:2}° — Scale: ({}x, {}x)",
						round(translation.x),
						round(translation.y),
						round(scale.x),
						round(scale.y)
					))
					.widget_holder(),
					TextLabel::new(format!("{}", instance.alpha_blending)).widget_holder(),
					TextLabel::new(instance.source_node_id.map_or_else(|| "-".to_string(), |id| format!("{}", id.0))).widget_holder(),
				]
			})
			.collect::<Vec<_>>();

		rows.insert(0, column_headings(&["", "instance", "transform", "alpha_blending", "source_node_id"]));

		let instances = vec![TextLabel::new("Instances:").widget_holder()];
		vec![LayoutGroup::Row { widgets: instances }, LayoutGroup::Table { rows }]
	}
}
