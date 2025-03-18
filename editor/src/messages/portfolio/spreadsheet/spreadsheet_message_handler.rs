use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget, WidgetLayout};
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeId;
use graphene_core::Context;
use graphene_core::instances::Instances;
use graphene_core::memo::IORecord;
use graphene_core::{Artboard, ArtboardGroupTable, GraphicElement};
use graphene_core::{
	GraphicGroupTable,
	vector::{VectorData, VectorDataTable},
};
use graphene_std::vector::InstanceId;
use std::any::Any;
use std::sync::Arc;

/// The spreadsheet UI allows for instance data to be previewed.
#[derive(Debug, Clone)]
pub struct SpreadsheetMessageHandler {
	/// Sets whether or not the spreadsheet is drawn.
	pub spreadsheet_view_open: bool,
	inspect_node: Option<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	path: Vec<InstanceId>,
}

impl Default for SpreadsheetMessageHandler {
	fn default() -> Self {
		Self {
			spreadsheet_view_open: true,
			inspect_node: None,
			introspected_data: None,
			path: Vec::new(),
		}
	}
}

impl MessageHandler<SpreadsheetMessage, ()> for SpreadsheetMessageHandler {
	fn process_message(&mut self, message: SpreadsheetMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			SpreadsheetMessage::SetOpen { open } => {
				self.spreadsheet_view_open = open;
				self.update_layout(responses);
			}
			SpreadsheetMessage::UpdateLayout { inspect_result } => {
				self.inspect_node = Some(inspect_result.inspect_node);
				self.introspected_data = inspect_result.introspected_data;
				self.update_layout(responses)
			}
			SpreadsheetMessage::PushInstance { id } => {
				self.path.push(id);
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
			desired_path: &mut self.path,
		};
		let layout = self
			.introspected_data
			.as_ref()
			.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data))
			.unwrap_or_else(|| Some(label("No data")))
			.unwrap_or_else(|| label("Failed to downcast data"));

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout { layout }),
			layout_target: LayoutTarget::Spreadsheet,
		});
	}
}

struct LayoutData<'a> {
	current_depth: usize,
	desired_path: &'a mut Vec<InstanceId>,
}

fn instances_layout<T: InstanceLayout>(instances: &Instances<T>, data: &mut LayoutData) -> Vec<LayoutGroup> {
	if let Some(id) = data.desired_path.get(data.current_depth).copied() {
		if let Some(instance) = instances.instances().find(|instance| *instance.id == id) {
			data.current_depth += 1;
			let result = instance.instance.layout(data);
			data.current_depth -= 1;
			return result;
		} else {
			warn!("Desired path truncated");
			data.desired_path.truncate(data.current_depth);
		}
	}

	let mut rows = instances
		.instances()
		.map(|instance| {
			let id = *instance.id;
			vec![
				TextLabel::new(format!("{}", instance.id.inner())).widget_holder(),
				TextButton::new(instance.instance.identifier())
					.on_update(move |_| SpreadsheetMessage::PushInstance { id }.into())
					.widget_holder(),
				TextLabel::new(format!("{}", instance.transform)).widget_holder(),
				TextLabel::new(format!("{:?}", instance.alpha_blending)).widget_holder(),
				TextLabel::new(instance.source_node_id.map_or_else(|| "-".to_string(), |id| format!("{}", id.0))).widget_holder(),
			]
		})
		.collect::<Vec<_>>();

	rows.insert(0, column_headings(&["id", "instance", "transform", "alpha_blending", "source_node_id"]));

	let instances = vec![TextLabel::new("Instances:").widget_holder()];
	vec![LayoutGroup::Row { widgets: instances }, LayoutGroup::Table { rows }]
}

fn generate_layout(introspected_data: &Arc<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// We simply try random types. TODO: better strategy.
	if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, ArtboardGroupTable>>() {
		Some(instances_layout(&io.output, data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), ArtboardGroupTable>>() {
		Some(instances_layout(&io.output, data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, VectorDataTable>>() {
		Some(instances_layout(&io.output, data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), VectorDataTable>>() {
		Some(instances_layout(&io.output, data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, GraphicGroupTable>>() {
		Some(instances_layout(&io.output, data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), GraphicGroupTable>>() {
		Some(instances_layout(&io.output, data))
	} else {
		None
	}
}

fn column_headings(value: &[&str]) -> Vec<WidgetHolder> {
	value.into_iter().map(|text| TextLabel::new(*text).widget_holder()).collect()
}

fn label(x: impl Into<String>) -> Vec<LayoutGroup> {
	let error = vec![TextLabel::new(x).widget_holder()];
	vec![LayoutGroup::Row { widgets: error }]
}

trait InstanceLayout {
	fn identifier(&self) -> String;
	fn layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup>;
}

impl InstanceLayout for GraphicElement {
	fn identifier(&self) -> String {
		match self {
			Self::GraphicGroup(instances) => format!("Instances<GraphicElement> (length={})", instances.len()),
			Self::VectorData(instances) => format!("Instances<VectorData> (length={})", instances.len()),
			Self::RasterFrame(_) => format!("RasterFrame"),
		}
	}
	fn layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Self::GraphicGroup(instances) => instances_layout(instances, data),
			Self::VectorData(instances) => instances_layout(instances, data),
			Self::RasterFrame(_) => label("Raster frame not supported"),
		}
	}
}

impl InstanceLayout for VectorData {
	fn identifier(&self) -> String {
		format!("Vector Data (points={}, segments={})", self.point_domain.ids().len(), self.segment_domain.ids().len())
	}
	fn layout(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		label("vector data")
	}
}

impl InstanceLayout for Artboard {
	fn identifier(&self) -> String {
		self.label.clone()
	}
	fn layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		instances_layout(&self.graphic_group, data)
	}
}
