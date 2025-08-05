use super::VectorDomain;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget, WidgetLayout};
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::Context;
use graphene_std::memo::IORecord;
use graphene_std::raster::Image;
use graphene_std::table::Table;
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Graphic};
use std::any::Any;
use std::sync::Arc;

/// The spreadsheet UI allows for graph data to be previewed.
#[derive(Default, Debug, Clone, ExtractField)]
pub struct SpreadsheetMessageHandler {
	/// Sets whether or not the spreadsheet is drawn.
	pub spreadsheet_view_open: bool,
	inspect_node: Option<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	element_path: Vec<usize>,
	viewing_vector_domain: VectorDomain,
}

#[message_handler_data]
impl MessageHandler<SpreadsheetMessage, ()> for SpreadsheetMessageHandler {
	fn process_message(&mut self, message: SpreadsheetMessage, responses: &mut VecDeque<Message>, _: ()) {
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

			SpreadsheetMessage::PushToElementPath { index } => {
				self.element_path.push(index);
				self.update_layout(responses);
			}
			SpreadsheetMessage::TruncateElementPath { len } => {
				self.element_path.truncate(len);
				self.update_layout(responses);
			}

			SpreadsheetMessage::ViewVectorDomain { domain } => {
				self.viewing_vector_domain = domain;
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
			desired_path: &mut self.element_path,
			breadcrumbs: Vec::new(),
			vector_domain: self.viewing_vector_domain,
		};
		let mut layout = self
			.introspected_data
			.as_ref()
			.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data))
			.unwrap_or_else(|| Some(label("No data")))
			.unwrap_or_else(|| label("Failed to downcast data"));

		if layout_data.breadcrumbs.len() > 1 {
			let breadcrumb = BreadcrumbTrailButtons::new(layout_data.breadcrumbs)
				.on_update(|&len| SpreadsheetMessage::TruncateElementPath { len: len as usize }.into())
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
	vector_domain: VectorDomain,
}

fn generate_layout(introspected_data: &Arc<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	// We simply try random types. TODO: better strategy.
	#[allow(clippy::manual_map)]
	if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Artboard>>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), Table<Artboard>>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Vector>>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), Table<Vector>>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Graphic>>>() {
		Some(io.output.layout_with_breadcrumb(data))
	} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), Table<Graphic>>>() {
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

trait TableRowLayout {
	fn type_name() -> &'static str;
	fn identifier(&self) -> String;
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(self.identifier());
		self.compute_layout(data)
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup>;
}

impl TableRowLayout for Graphic {
	fn type_name() -> &'static str {
		"Graphic"
	}
	fn identifier(&self) -> String {
		match self {
			Self::Group(group) => group.identifier(),
			Self::Vector(vector) => vector.identifier(),
			Self::RasterCPU(_) => "Raster (on CPU)".to_string(),
			Self::RasterGPU(_) => "Raster (on GPU)".to_string(),
		}
	}
	// Don't put a breadcrumb for Graphic
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.compute_layout(data)
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Self::Group(table) => table.layout_with_breadcrumb(data),
			Self::Vector(table) => table.layout_with_breadcrumb(data),
			Self::RasterCPU(_) => label("Raster is not supported"),
			Self::RasterGPU(_) => label("Raster is not supported"),
		}
	}
}

impl TableRowLayout for Vector {
	fn type_name() -> &'static str {
		"Vector"
	}
	fn identifier(&self) -> String {
		format!(
			"Vector ({} point{}, {} segment{})",
			self.point_domain.ids().len(),
			if self.point_domain.ids().len() == 1 { "" } else { "s" },
			self.segment_domain.ids().len(),
			if self.segment_domain.ids().len() == 1 { "" } else { "s" }
		)
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		let colinear = self.colinear_manipulators.iter().map(|[a, b]| format!("[{a} / {b}]")).collect::<Vec<_>>().join(", ");
		let colinear = if colinear.is_empty() { "None" } else { &colinear };
		let style = vec![
			TextLabel::new(format!(
				"{}\n\nColinear Handle IDs: {}\n\nUpstream Group Table: {}",
				self.style,
				colinear,
				if self.upstream_group.is_some() { "Yes" } else { "No" }
			))
			.multiline(true)
			.widget_holder(),
		];

		let domain_entries = [VectorDomain::Points, VectorDomain::Segments, VectorDomain::Regions]
			.into_iter()
			.map(|domain| {
				RadioEntryData::new(format!("{domain:?}"))
					.label(format!("{domain:?}"))
					.on_update(move |_| SpreadsheetMessage::ViewVectorDomain { domain }.into())
			})
			.collect();
		let domain = vec![RadioInput::new(domain_entries).selected_index(Some(data.vector_domain as u32)).widget_holder()];

		let mut table_rows = Vec::new();
		match data.vector_domain {
			VectorDomain::Points => {
				table_rows.push(column_headings(&["", "position"]));
				table_rows.extend(
					self.point_domain
						.iter()
						.map(|(id, position)| vec![TextLabel::new(format!("{}", id.inner())).widget_holder(), TextLabel::new(format!("{}", position)).widget_holder()]),
				);
			}
			VectorDomain::Segments => {
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
			VectorDomain::Regions => {
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

impl TableRowLayout for Image<Color> {
	fn type_name() -> &'static str {
		"Image"
	}
	fn identifier(&self) -> String {
		format!("Image ({}x{})", self.width, self.height)
	}
	fn compute_layout(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let rows = vec![vec![TextLabel::new(format!("Image ({}x{})", self.width, self.height)).widget_holder()]];
		vec![LayoutGroup::Table { rows }]
	}
}

impl TableRowLayout for Artboard {
	fn type_name() -> &'static str {
		"Artboard"
	}
	fn identifier(&self) -> String {
		self.label.clone()
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.group.compute_layout(data)
	}
}

impl<T: TableRowLayout> TableRowLayout for Table<T> {
	fn type_name() -> &'static str {
		"Table"
	}
	fn identifier(&self) -> String {
		format!("Table<{}> ({} row{})", T::type_name(), self.len(), if self.len() == 1 { "" } else { "s" })
	}
	fn compute_layout(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		if let Some(index) = data.desired_path.get(data.current_depth).copied() {
			if let Some(row) = self.get(index) {
				data.current_depth += 1;
				let result = row.element.layout_with_breadcrumb(data);
				data.current_depth -= 1;
				return result;
			} else {
				warn!("Desired path truncated");
				data.desired_path.truncate(data.current_depth);
			}
		}

		let mut rows = self
			.iter()
			.enumerate()
			.map(|(index, row)| {
				let (scale, angle, translation) = row.transform.to_scale_angle_translation();
				let rotation = if angle == -0. { 0. } else { angle.to_degrees() };
				let round = |x: f64| (x * 1e3).round() / 1e3;
				vec![
					TextLabel::new(format!("{index}")).widget_holder(),
					TextButton::new(row.element.identifier())
						.on_update(move |_| SpreadsheetMessage::PushToElementPath { index }.into())
						.widget_holder(),
					TextLabel::new(format!(
						"Location: ({} px, {} px) — Rotation: {rotation:2}° — Scale: ({}x, {}x)",
						round(translation.x),
						round(translation.y),
						round(scale.x),
						round(scale.y)
					))
					.widget_holder(),
					TextLabel::new(format!("{}", row.alpha_blending)).widget_holder(),
					TextLabel::new(row.source_node_id.map_or_else(|| "-".to_string(), |id| format!("{}", id.0))).widget_holder(),
				]
			})
			.collect::<Vec<_>>();

		rows.insert(0, column_headings(&["", "element", "transform", "alpha_blending", "source_node_id"]));

		vec![LayoutGroup::Table { rows }]
	}
}
