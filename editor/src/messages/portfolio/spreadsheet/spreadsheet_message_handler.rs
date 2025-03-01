use super::VectorDataDomain;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget, WidgetLayout};
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeId;
use graphene_core::Context;
use graphene_core::GraphicGroupTable;
use graphene_core::instances::Instances;
use graphene_core::memo::IORecord;
use graphene_core::vector::{VectorData, VectorDataTable};
use graphene_core::{Artboard, ArtboardGroupTable, GraphicElement};
use std::any::Any;
use std::sync::Arc;

/// The spreadsheet UI allows for instance data to be previewed.
#[derive(Default, Debug, Clone)]
pub struct SpreadsheetMessageHandler {
	/// Sets whether or not the spreadsheet is drawn.
	pub spreadsheet_view_open: bool,
	inspect_node: Option<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	instances_path: Vec<usize>,
	viewing_vector_data_domain: VectorDataDomain,
}

impl MessageHandler<SpreadsheetMessage, ()> for SpreadsheetMessageHandler {
	fn process_message(&mut self, message: SpreadsheetMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			SpreadsheetMessage::ToggleOpen => {
				self.spreadsheet_view_open = !self.spreadsheet_view_open;
				// Run the graph to grab the data
				if self.spreadsheet_view_open {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				// Update checked UI state for open
				responses.add(MenuBarMessage::SendLayout);
				self.update_layout(responses);
			}

			SpreadsheetMessage::UpdateLayout { mut inspect_result } => {
				self.inspect_node = Some(inspect_result.inspect_node);
				self.introspected_data = inspect_result.take_data();
				self.update_layout(responses)
			}

			SpreadsheetMessage::PushToInstancePath { index } => {
				self.instances_path.push(index);
				self.update_layout(responses);
			}
			SpreadsheetMessage::TruncateInstancePath { len } => {
				self.instances_path.truncate(len);
				self.update_layout(responses);
			}

			SpreadsheetMessage::ViewVectorDataDomain { domain } => {
				self.viewing_vector_data_domain = domain;
				self.update_layout(responses);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(SpreadsheetMessage;)
	}
}

impl SpreadsheetMessageHandler {
	fn update_layout(&mut self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateSpreadsheetState {
			node: self.inspect_node,
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
		let mut layout = self
			.introspected_data
			.as_ref()
			.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data))
			.unwrap_or_else(|| Some(label("No data")))
			.unwrap_or_else(|| label("Failed to downcast data"));

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

fn generate_layout(introspected_data: &Arc<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// We simply try random types. TODO: better strategy.
	#[allow(clippy::manual_map)]
	if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, ArtboardGroupTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), ArtboardGroupTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, VectorDataTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), VectorDataTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, GraphicGroupTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), GraphicGroupTable>>() {
		Some(io.output.layout_with_breadcrumb(data))
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
			Self::RasterFrame(_) => "RasterFrame".to_string(),
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
			Self::RasterFrame(_) => label("Raster frame not supported"),
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
		let mut rows = Vec::new();
		match data.vector_data_domain {
			VectorDataDomain::Points => {
				rows.push(column_headings(&["", "position"]));
				rows.extend(
					self.point_domain
						.iter()
						.map(|(id, position)| vec![TextLabel::new(format!("{}", id.inner())).widget_holder(), TextLabel::new(format!("{}", position)).widget_holder()]),
				);
			}
			VectorDataDomain::Segments => {
				rows.push(column_headings(&["", "start_index", "end_index", "handles"]));
				rows.extend(self.segment_domain.iter().map(|(id, start, end, handles)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).widget_holder(),
						TextLabel::new(format!("{}", start)).widget_holder(),
						TextLabel::new(format!("{}", end)).widget_holder(),
						TextLabel::new(format!("{:?}", handles)).widget_holder(),
					]
				}));
			}
			VectorDataDomain::Regions => {
				rows.push(column_headings(&["", "segment_range", "fill"]));
				rows.extend(self.region_domain.iter().map(|(id, segment_range, fill)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).widget_holder(),
						TextLabel::new(format!("{:?}", segment_range)).widget_holder(),
						TextLabel::new(format!("{}", fill.inner())).widget_holder(),
					]
				}));
			}
		}

		let entries = [VectorDataDomain::Points, VectorDataDomain::Segments, VectorDataDomain::Regions]
			.into_iter()
			.map(|domain| {
				RadioEntryData::new(format!("{domain:?}"))
					.label(format!("{domain:?}"))
					.on_update(move |_| SpreadsheetMessage::ViewVectorDataDomain { domain }.into())
			})
			.collect();

		let domain = vec![RadioInput::new(entries).selected_index(Some(data.vector_data_domain as u32)).widget_holder()];
		vec![LayoutGroup::Row { widgets: domain }, LayoutGroup::Table { rows }]
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
			.instances()
			.enumerate()
			.map(|(index, instance)| {
				vec![
					TextLabel::new(format!("{}", index)).widget_holder(),
					TextButton::new(instance.instance.identifier())
						.on_update(move |_| SpreadsheetMessage::PushToInstancePath { index }.into())
						.widget_holder(),
					TextLabel::new(format!("{}", instance.transform)).widget_holder(),
					TextLabel::new(format!("{:?}", instance.alpha_blending)).widget_holder(),
					TextLabel::new(instance.source_node_id.map_or_else(|| "-".to_string(), |id| format!("{}", id.0))).widget_holder(),
				]
			})
			.collect::<Vec<_>>();

		rows.insert(0, column_headings(&["", "instance", "transform", "alpha_blending", "source_node_id"]));

		let instances = vec![TextLabel::new("Instances:").widget_holder()];
		vec![LayoutGroup::Row { widgets: instances }, LayoutGroup::Table { rows }]
	}
}
