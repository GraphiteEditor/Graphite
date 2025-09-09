use super::VectorTableTab;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget, WidgetLayout};
use crate::messages::portfolio::document::data_panel::DataPanelMessage;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{Affine2, Vec2};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::Context;
use graphene_std::gradient::GradientStops;
use graphene_std::memo::IORecord;
use graphene_std::raster_types::{CPU, GPU, Raster};
use graphene_std::table::Table;
use graphene_std::vector::Vector;
use graphene_std::vector::style::{Fill, FillChoice};
use graphene_std::{Artboard, Graphic};
use std::any::Any;
use std::sync::Arc;

#[derive(ExtractField)]
pub struct DataPanelMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub data_panel_open: bool,
}

/// The data panel allows for graph data to be previewed.
#[derive(Default, Debug, Clone, ExtractField)]
pub struct DataPanelMessageHandler {
	introspected_node: Option<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	element_path: Vec<usize>,
	active_vector_table_tab: VectorTableTab,
}

#[message_handler_data]
impl MessageHandler<DataPanelMessage, DataPanelMessageContext<'_>> for DataPanelMessageHandler {
	fn process_message(&mut self, message: DataPanelMessage, responses: &mut VecDeque<Message>, context: DataPanelMessageContext) {
		match message {
			DataPanelMessage::UpdateLayout { mut inspect_result } => {
				self.introspected_node = Some(inspect_result.inspect_node);
				self.introspected_data = inspect_result.take_data();
				self.update_layout(responses, context);
			}
			DataPanelMessage::ClearLayout => {
				self.introspected_node = None;
				self.introspected_data = None;
				self.element_path.clear();
				self.active_vector_table_tab = VectorTableTab::default();
				self.update_layout(responses, context);
			}

			DataPanelMessage::PushToElementPath { index } => {
				self.element_path.push(index);
				self.update_layout(responses, context);
			}
			DataPanelMessage::TruncateElementPath { len } => {
				self.element_path.truncate(len);
				self.update_layout(responses, context);
			}

			DataPanelMessage::ViewVectorTableTab { tab } => {
				self.active_vector_table_tab = tab;
				self.update_layout(responses, context);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(DataPanelMessage;)
	}
}

impl DataPanelMessageHandler {
	fn update_layout(&mut self, responses: &mut VecDeque<Message>, context: DataPanelMessageContext<'_>) {
		let DataPanelMessageContext { network_interface, .. } = context;

		let mut layout_data = LayoutData {
			current_depth: 0,
			desired_path: &mut self.element_path,
			breadcrumbs: Vec::new(),
			vector_table_tab: self.active_vector_table_tab,
		};

		// Main data visualization
		let mut layout = self
			.introspected_data
			.as_ref()
			.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data).unwrap_or_else(|| label("Visualization of this data type is not yet supported")))
			.unwrap_or_default();

		let mut widgets = Vec::new();

		// Selected layer/node name
		if let Some(node_id) = self.introspected_node {
			let is_layer = network_interface.is_layer(&node_id, &[]);

			widgets.extend([
				if is_layer {
					IconLabel::new("Layer").tooltip("Name of the selected layer").widget_holder()
				} else {
					IconLabel::new("Node").tooltip("Name of the selected node").widget_holder()
				},
				Separator::new(SeparatorType::Related).widget_holder(),
				TextInput::new(network_interface.display_name(&node_id, &[]))
					.tooltip(if is_layer { "Name of the selected layer" } else { "Name of the selected node" })
					.on_update(move |text_input| {
						NodeGraphMessage::SetDisplayName {
							node_id,
							alias: text_input.value.clone(),
							skip_adding_history_step: false,
						}
						.into()
					})
					.max_width(200)
					.widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
			]);
		}

		// Element path breadcrumbs
		if !layout_data.breadcrumbs.is_empty() {
			let breadcrumb = BreadcrumbTrailButtons::new(layout_data.breadcrumbs)
				.on_update(|&len| DataPanelMessage::TruncateElementPath { len: len as usize }.into())
				.widget_holder();
			widgets.push(breadcrumb);
		}

		if !widgets.is_empty() {
			layout.insert(0, LayoutGroup::Row { widgets });
		}

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout { layout }),
			layout_target: LayoutTarget::DataPanel,
		});
	}
}

struct LayoutData<'a> {
	current_depth: usize,
	desired_path: &'a mut Vec<usize>,
	breadcrumbs: Vec<String>,
	vector_table_tab: VectorTableTab,
}

macro_rules! generate_layout_downcast {
	($introspected_data:expr, $data:expr, [ $($ty:ty),* $(,)? ]) => {
		if false { None }
		$(
			else if let Some(io) = $introspected_data.downcast_ref::<IORecord<Context, $ty>>() {
				Some(io.output.layout_with_breadcrumb($data))
			}
		)*
		else { None }
	}
}
// TODO: We simply try all these types sequentially. Find a better strategy.
fn generate_layout(introspected_data: &Arc<dyn std::any::Any + Send + Sync + 'static>, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	generate_layout_downcast!(introspected_data, data, [
		Table<Artboard>,
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
		f64,
		u32,
		u64,
		bool,
		String,
		Option<f64>,
		DVec2,
		DAffine2,
	])
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
		self.element_page(data)
	}
	fn element_widget(&self, index: usize) -> WidgetHolder {
		TextButton::new(self.identifier())
			.on_update(move |_| DataPanelMessage::PushToElementPath { index }.into())
			.narrow(true)
			.widget_holder()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![]
	}
}

impl<T: TableRowLayout> TableRowLayout for Table<T> {
	fn type_name() -> &'static str {
		"Table"
	}
	fn identifier(&self) -> String {
		format!("Table<{}> ({} row{})", T::type_name(), self.len(), if self.len() == 1 { "" } else { "s" })
	}
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
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
				vec![
					TextLabel::new(format!("{index}")).narrow(true).widget_holder(),
					row.element.element_widget(index),
					TextLabel::new(format_transform_matrix(row.transform)).narrow(true).widget_holder(),
					TextLabel::new(format!("{}", row.alpha_blending)).narrow(true).widget_holder(),
					TextLabel::new(row.source_node_id.map_or_else(|| "-".to_string(), |id| format!("{}", id.0)))
						.narrow(true)
						.widget_holder(),
				]
			})
			.collect::<Vec<_>>();

		rows.insert(0, column_headings(&["", "element", "transform", "alpha_blending", "source_node_id"]));

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
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.content.element_page(data)
	}
}

impl TableRowLayout for Graphic {
	fn type_name() -> &'static str {
		"Graphic"
	}
	fn identifier(&self) -> String {
		match self {
			Self::Graphic(table) => table.identifier(),
			Self::Vector(table) => table.identifier(),
			Self::RasterCPU(table) => table.identifier(),
			Self::RasterGPU(table) => table.identifier(),
			Self::Color(table) => table.identifier(),
			Self::Gradient(table) => table.identifier(),
		}
	}
	// Don't put a breadcrumb for Graphic
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		self.element_page(data)
	}
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Self::Graphic(table) => table.layout_with_breadcrumb(data),
			Self::Vector(table) => table.layout_with_breadcrumb(data),
			Self::RasterCPU(table) => table.layout_with_breadcrumb(data),
			Self::RasterGPU(table) => table.layout_with_breadcrumb(data),
			Self::Color(table) => table.layout_with_breadcrumb(data),
			Self::Gradient(table) => table.layout_with_breadcrumb(data),
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
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		let table_tab_entries = [VectorTableTab::Properties, VectorTableTab::Points, VectorTableTab::Segments, VectorTableTab::Regions]
			.into_iter()
			.map(|tab| {
				RadioEntryData::new(format!("{tab:?}"))
					.label(format!("{tab:?}"))
					.on_update(move |_| DataPanelMessage::ViewVectorTableTab { tab }.into())
			})
			.collect();
		let table_tabs = vec![RadioInput::new(table_tab_entries).selected_index(Some(data.vector_table_tab as u32)).widget_holder()];

		let mut table_rows = Vec::new();
		match data.vector_table_tab {
			VectorTableTab::Properties => {
				table_rows.push(column_headings(&["property", "value"]));

				match self.style.fill.clone() {
					Fill::None => table_rows.push(vec![
						TextLabel::new("Fill").narrow(true).widget_holder(),
						ColorInput::new(FillChoice::None).disabled(true).menu_direction(Some(MenuDirection::Top)).narrow(true).widget_holder(),
					]),
					Fill::Solid(color) => table_rows.push(vec![
						TextLabel::new("Fill").narrow(true).widget_holder(),
						ColorInput::new(FillChoice::Solid(color))
							.disabled(true)
							.menu_direction(Some(MenuDirection::Top))
							.narrow(true)
							.widget_holder(),
					]),
					Fill::Gradient(gradient) => {
						table_rows.push(vec![
							TextLabel::new("Fill").narrow(true).widget_holder(),
							ColorInput::new(FillChoice::Gradient(gradient.stops))
								.disabled(true)
								.menu_direction(Some(MenuDirection::Top))
								.narrow(true)
								.widget_holder(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient Type").narrow(true).widget_holder(),
							TextLabel::new(gradient.gradient_type.to_string()).narrow(true).widget_holder(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient Start").narrow(true).widget_holder(),
							TextLabel::new(format_dvec2(gradient.start)).narrow(true).widget_holder(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient End").narrow(true).widget_holder(),
							TextLabel::new(format_dvec2(gradient.end)).narrow(true).widget_holder(),
						]);
					}
				}

				if let Some(stroke) = self.style.stroke.clone() {
					let color = if let Some(color) = stroke.color { FillChoice::Solid(color) } else { FillChoice::None };
					table_rows.push(vec![
						TextLabel::new("Stroke").narrow(true).widget_holder(),
						ColorInput::new(color).disabled(true).menu_direction(Some(MenuDirection::Top)).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Weight").narrow(true).widget_holder(),
						TextLabel::new(format!("{} px", stroke.weight)).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Dash Lengths").narrow(true).widget_holder(),
						TextLabel::new(if stroke.dash_lengths.is_empty() {
							"-".to_string()
						} else {
							format!("[{}]", stroke.dash_lengths.iter().map(|x| format!("{x} px")).collect::<Vec<_>>().join(", "))
						})
						.narrow(true)
						.widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Dash Offset").narrow(true).widget_holder(),
						TextLabel::new(format!("{}", stroke.dash_offset)).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Cap").narrow(true).widget_holder(),
						TextLabel::new(stroke.cap.to_string()).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Join").narrow(true).widget_holder(),
						TextLabel::new(stroke.join.to_string()).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Join Miter Limit").narrow(true).widget_holder(),
						TextLabel::new(format!("{}", stroke.join_miter_limit)).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Align").narrow(true).widget_holder(),
						TextLabel::new(stroke.align.to_string()).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Transform").narrow(true).widget_holder(),
						TextLabel::new(format_transform_matrix(&stroke.transform)).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Non-Scaling").narrow(true).widget_holder(),
						TextLabel::new((if stroke.non_scaling { "Yes" } else { "No" }).to_string()).narrow(true).widget_holder(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Paint Order").narrow(true).widget_holder(),
						TextLabel::new(stroke.paint_order.to_string()).narrow(true).widget_holder(),
					]);
				}

				let colinear = self.colinear_manipulators.iter().map(|[a, b]| format!("[{a} / {b}]")).collect::<Vec<_>>().join(", ");
				let colinear = if colinear.is_empty() { "-".to_string() } else { colinear };
				table_rows.push(vec![
					TextLabel::new("Colinear Handle IDs").narrow(true).widget_holder(),
					TextLabel::new(colinear).narrow(true).widget_holder(),
				]);

				table_rows.push(vec![
					TextLabel::new("Upstream Nested Layers").narrow(true).widget_holder(),
					TextLabel::new(if self.upstream_nested_layers.is_some() {
						"Yes (this preserves references to its upstream nested layers for editing by tools)"
					} else {
						"No (this doesn't preserve references to its upstream nested layers for editing by tools)"
					})
					.narrow(true)
					.widget_holder(),
				]);
			}
			VectorTableTab::Points => {
				table_rows.push(column_headings(&["", "position"]));
				table_rows.extend(self.point_domain.iter().map(|(id, position)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_holder(),
						TextLabel::new(format!("{position}")).narrow(true).widget_holder(),
					]
				}));
			}
			VectorTableTab::Segments => {
				table_rows.push(column_headings(&["", "start_index", "end_index", "handles"]));
				table_rows.extend(self.segment_domain.iter().map(|(id, start, end, handles)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_holder(),
						TextLabel::new(format!("{start}")).narrow(true).widget_holder(),
						TextLabel::new(format!("{end}")).narrow(true).widget_holder(),
						TextLabel::new(format!("{handles:?}")).narrow(true).widget_holder(),
					]
				}));
			}
			VectorTableTab::Regions => {
				table_rows.push(column_headings(&["", "segment_range", "fill"]));
				table_rows.extend(self.region_domain.iter().map(|(id, segment_range, fill)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_holder(),
						TextLabel::new(format!("{segment_range:?}")).narrow(true).widget_holder(),
						TextLabel::new(format!("{}", fill.inner())).narrow(true).widget_holder(),
					]
				}));
			}
		}

		vec![LayoutGroup::Row { widgets: table_tabs }, LayoutGroup::Table { rows: table_rows }]
	}
}

impl TableRowLayout for Raster<CPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({}x{})", self.width, self.height)
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let base64_string = self.data().base64_string.clone().unwrap_or_else(|| {
			use base64::Engine;

			let output = self.data().to_png();
			let preamble = "data:image/png;base64,";
			let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
			base64_string.push_str(preamble);
			base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
			base64_string
		});

		let widgets = vec![ImageLabel::new(base64_string).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for Raster<GPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({}x{})", self.data().width(), self.data().height())
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new("Raster is a texture on the GPU and cannot currently be displayed here").widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for Color {
	fn type_name() -> &'static str {
		"Color"
	}
	fn identifier(&self) -> String {
		format!("Color (#{})", self.to_gamma_srgb().to_rgba_hex_srgb())
	}
	fn element_widget(&self, _index: usize) -> WidgetHolder {
		ColorInput::new(FillChoice::Solid(*self))
			.disabled(true)
			.menu_direction(Some(MenuDirection::Top))
			.narrow(true)
			.widget_holder()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![self.element_widget(0)];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for GradientStops {
	fn type_name() -> &'static str {
		"Gradient"
	}
	fn identifier(&self) -> String {
		format!("Gradient ({} stops)", self.0.len())
	}
	fn element_widget(&self, _index: usize) -> WidgetHolder {
		ColorInput::new(FillChoice::Gradient(self.clone()))
			.menu_direction(Some(MenuDirection::Top))
			.disabled(true)
			.narrow(true)
			.widget_holder()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![self.element_widget(0)];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for f64 {
	fn type_name() -> &'static str {
		"Number (f64)"
	}
	fn identifier(&self) -> String {
		"Number (f64)".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(self.to_string()).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for u32 {
	fn type_name() -> &'static str {
		"Number (u32)"
	}
	fn identifier(&self) -> String {
		"Number (u32)".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(self.to_string()).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for u64 {
	fn type_name() -> &'static str {
		"Number (u64)"
	}
	fn identifier(&self) -> String {
		"Number (u64)".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(self.to_string()).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for bool {
	fn type_name() -> &'static str {
		"Bool"
	}
	fn identifier(&self) -> String {
		"Bool".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(self.to_string()).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for String {
	fn type_name() -> &'static str {
		"String"
	}
	fn identifier(&self) -> String {
		"String".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextAreaInput::new(self.to_string()).disabled(true).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for Option<f64> {
	fn type_name() -> &'static str {
		"Option<f64>"
	}
	fn identifier(&self) -> String {
		"Option<f64>".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(format!("{self:?}")).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for DVec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(format!("({}, {})", self.x, self.y)).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for Vec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(format!("({}, {})", self.x, self.y)).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for DAffine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new(format_transform_matrix(self)).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

impl TableRowLayout for Affine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let matrix = DAffine2::from_cols_array(&self.to_cols_array().map(|x| x as f64));
		let widgets = vec![TextLabel::new(format_transform_matrix(&matrix)).widget_holder()];
		vec![LayoutGroup::Row { widgets }]
	}
}

fn format_transform_matrix(transform: &DAffine2) -> String {
	let (scale, angle, translation) = transform.to_scale_angle_translation();
	let rotation = if angle == -0. { 0. } else { angle.to_degrees() };
	let round = |x: f64| (x * 1e3).round() / 1e3;

	format!(
		"Location: ({} px, {} px) — Rotation: {rotation:2}° — Scale: ({}x, {}x)",
		round(translation.x),
		round(translation.y),
		round(scale.x),
		round(scale.y)
	)
}

fn format_dvec2(value: DVec2) -> String {
	let round = |x: f64| (x * 1e3).round() / 1e3;
	format!("({} px, {} px)", round(value.x), round(value.y))
}
