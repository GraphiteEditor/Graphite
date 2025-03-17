use super::utility_types::layout_widget::{Layout, LayoutGroup, WidgetLayout};
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use crate::node_graph_executor::InspectResult;
use graphene_core::Context;
use graphene_core::instances::Instances;
use graphene_core::memo::IORecord;
use graphene_core::{GraphicGroupTable, vector::VectorDataTable};
use std::sync::Arc;

pub fn update_layout(responses: &mut VecDeque<Message>, inspect_result: InspectResult) {
	responses.add(FrontendMessage::UpdateSpreadsheetState {
		node: inspect_result.inspect_node,
		open: true,
	});

	let introspected_data = inspect_result.introspected_data;

	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout {
			layout: generate_layout(introspected_data),
		}),
		layout_target: super::utility_types::layout_widget::LayoutTarget::Spreadsheet,
	});
}

fn instances_layout<T: std::fmt::Debug>(instances: &Instances<T>) -> Vec<LayoutGroup> {
	let rows = instances
		.instances()
		.map(|instance| {
			vec![
				TextLabel::new(format!("{:?}", instance.id)).widget_holder(),
				TextLabel::new(format!("{:?}", instance.instance)).widget_holder(),
				TextLabel::new(format!("{:?}", instance.transform)).widget_holder(),
				TextLabel::new(format!("{:?}", instance.alpha_blending)).widget_holder(),
				TextLabel::new(format!("{:?}", instance.source_node_id)).widget_holder(),
			]
		})
		.collect::<Vec<_>>();

	let instances = vec![TextLabel::new("Instances:").widget_holder()];
	vec![LayoutGroup::Row { widgets: instances }, LayoutGroup::Table { rows }]
}

fn label(x: impl Into<String>) -> Vec<LayoutGroup> {
	let error = vec![TextLabel::new(x).widget_holder()];
	vec![LayoutGroup::Row { widgets: error }]
}

fn generate_layout(introspected_data: Arc<dyn std::any::Any + Send + Sync + 'static>) -> Vec<LayoutGroup> {
	// We simply try random types. TODO: better strategy.
	if let Some(_io) = introspected_data.downcast_ref::<IORecord<Context, graphene_core::GraphicElement>>() {
		label("Graphic elements not supported")
	} else if let Some(_io) = introspected_data.downcast_ref::<IORecord<(), graphene_core::GraphicElement>>() {
		label("Graphic elements not supported")
	} else if let Some(_io) = introspected_data.downcast_ref::<IORecord<Context, graphene_core::Artboard>>() {
		label("Artboard not supported")
	} else if let Some(_io) = introspected_data.downcast_ref::<IORecord<(), graphene_core::Artboard>>() {
		label("Artboard not supported")
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, VectorDataTable>>() {
		instances_layout(&io.output)
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), VectorDataTable>>() {
		instances_layout(&io.output)
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, GraphicGroupTable>>() {
		instances_layout(&io.output)
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), GraphicGroupTable>>() {
		instances_layout(&io.output)
	} else {
		label("Failed to downcast data")
	}
}
