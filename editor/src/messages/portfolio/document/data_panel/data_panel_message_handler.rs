use super::VectorTableTab;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, LayoutTarget};
use crate::messages::portfolio::document::data_panel::{DataPanelMessage, PathStep};
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::{Affine2, DAffine2, Vec2};
use graph_craft::document::NodeId;
use graphene_std::Context;
use graphene_std::gradient::GradientStops;
use graphene_std::memo::IORecord;
use graphene_std::raster_types::{CPU, GPU, Raster};
use graphene_std::table::Table;
use graphene_std::vector::Vector;
use graphene_std::vector::style::{Fill, FillChoice};
use graphene_std::{AlphaBlending, Color};
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
	/// Full path from the root network to the introspected node, with the node itself as the last element.
	/// Empty when nothing is being introspected.
	introspected_node_path: Vec<NodeId>,
	introspected_data: Option<Arc<dyn Any + Send + Sync>>,
	element_path: Vec<PathStep>,
	active_vector_table_tab: VectorTableTab,
}

#[message_handler_data]
impl MessageHandler<DataPanelMessage, DataPanelMessageContext<'_>> for DataPanelMessageHandler {
	fn process_message(&mut self, message: DataPanelMessage, responses: &mut VecDeque<Message>, context: DataPanelMessageContext) {
		match message {
			DataPanelMessage::UpdateLayout { mut inspect_result } => {
				self.introspected_data = inspect_result.take_data();
				self.introspected_node_path = inspect_result.inspect_node_path;
				self.update_layout(responses, context);
			}
			DataPanelMessage::ClearLayout => {
				self.introspected_node_path.clear();
				self.introspected_data = None;
				self.element_path.clear();
				self.active_vector_table_tab = VectorTableTab::default();
				self.update_layout(responses, context);
			}
			DataPanelMessage::Refresh => {
				// Re-render against the current network_interface without disturbing introspected_data or the breadcrumb path.
				if self.introspected_data.is_some() {
					self.update_layout(responses, context);
				}
			}

			DataPanelMessage::PushToElementPath { step } => {
				self.element_path.push(step);
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
			network_interface: &*network_interface,
			breadcrumbs: Vec::new(),
			vector_table_tab: self.active_vector_table_tab,
		};

		// Main data visualization
		let mut layout = Layout(
			self.introspected_data
				.as_ref()
				.map(|instrospected_data| generate_layout(instrospected_data, &mut layout_data).unwrap_or_else(|| label("Visualization of this data type is not yet supported")))
				.unwrap_or_default(),
		);

		let mut widgets = Vec::new();

		// Selected layer/node name
		if let Some((node_id, parent_path)) = self.introspected_node_path.split_last() {
			let node_id = *node_id;
			let is_layer = network_interface.is_layer(&node_id, parent_path);
			let parent_path_owned = parent_path.to_vec();

			widgets.extend([
				if is_layer {
					IconLabel::new("Layer").tooltip_description("Name of the selected layer.").widget_instance()
				} else {
					IconLabel::new("Node").tooltip_description("Name of the selected node.").widget_instance()
				},
				Separator::new(SeparatorStyle::Related).widget_instance(),
				TextInput::new(network_interface.display_name(&node_id, parent_path))
					.tooltip_description(if is_layer { "Name of the selected layer." } else { "Name of the selected node." })
					.on_update(move |text_input| {
						NodeGraphMessage::SetDisplayName {
							node_id,
							network_path: parent_path_owned.clone(),
							alias: text_input.value.clone(),
							skip_adding_history_step: false,
						}
						.into()
					})
					.max_width(200)
					.widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			]);
		}

		// Element path breadcrumbs
		if !layout_data.breadcrumbs.is_empty() {
			let breadcrumb = BreadcrumbTrailButtons::new(layout_data.breadcrumbs)
				.on_update(|&len| DataPanelMessage::TruncateElementPath { len: len as usize }.into())
				.widget_instance();
			widgets.push(breadcrumb);
		}

		if !widgets.is_empty() {
			layout.0.insert(0, LayoutGroup::row(widgets));
		}

		responses.add(LayoutMessage::SendLayout {
			layout,
			layout_target: LayoutTarget::DataPanel,
		});
	}
}

struct LayoutData<'a> {
	current_depth: usize,
	desired_path: &'a mut Vec<PathStep>,
	network_interface: &'a NodeNetworkInterface,
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
		Table<String>,
		Table<NodeId>,
		Table<f64>,
		Table<u8>,
		GradientStops,
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

fn column_headings(value: &[&str]) -> Vec<WidgetInstance> {
	value.iter().map(|text| TextLabel::new(*text).widget_instance()).collect()
}

fn label(x: impl Into<String>) -> Vec<LayoutGroup> {
	let error = vec![TextLabel::new(x).widget_instance()];
	vec![LayoutGroup::row(error)]
}

trait TableRowLayout {
	fn type_name() -> &'static str;
	fn identifier(&self) -> String;
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(self.identifier());
		self.element_page(data)
	}
	/// Renders this value as a single inline widget inside a row of a Vec/Table.
	/// `target` is the [`PathStep`] to push when the cell is clicked to drill into the value.
	/// `data` provides shared context (notably `network_interface`) for types whose label or content
	/// depends on lookup beyond their own value (e.g. `NodeId` resolving a node's display name).
	/// The default is a button labeled with `identifier()`. Types whose values are best shown
	/// inline (colors, transforms, primitives, etc.) override this to ignore `target` and
	/// return a richer non-navigating widget.
	fn cell_widget(&self, target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextButton::new(self.identifier())
			.on_update(move |_| DataPanelMessage::PushToElementPath { step: target.clone() }.into())
			.narrow(true)
			.widget_instance()
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
		format!("Table<{}> ({} element{})", T::type_name(), self.len(), if self.len() == 1 { "" } else { "s" })
	}
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		if let Some(step) = data.desired_path.get(data.current_depth).cloned() {
			match step {
				PathStep::Element(index) => {
					if let Some(element) = self.element(index) {
						data.current_depth += 1;
						let result = element.layout_with_breadcrumb(data);
						data.current_depth -= 1;
						return result;
					} else {
						warn!("Desired path truncated");
						data.desired_path.truncate(data.current_depth);
					}
				}
				PathStep::Attribute { row, key } => {
					if let Some(any) = self.attribute_any(&key, row) {
						data.current_depth += 1;
						if let Some(result) = drilldown_attribute_layout(any, data) {
							data.current_depth -= 1;
							return result;
						}
						data.current_depth -= 1;
						warn!("Drilldown unsupported for attribute {key:?}");
					}
					data.desired_path.truncate(data.current_depth);
				}
			}
		}

		let attribute_keys: Vec<String> = self.attribute_keys().map(str::to_string).collect();

		let mut rows = (0..self.len())
			.map(|index| {
				let element = self.element(index).unwrap();
				let mut cells = vec![TextLabel::new(format!("{index}")).narrow(true).widget_instance(), element.cell_widget(PathStep::Element(index), data)];
				for key in &attribute_keys {
					let target = PathStep::Attribute { row: index, key: key.clone() };
					let widget = self.attribute_any(key, index).and_then(|any| dispatch_cell_widget(any, target, data)).unwrap_or_else(|| {
						let text = self.attribute_display_value(key, index, |_| None).unwrap_or_else(|| "-".to_string());
						TextLabel::new(text).narrow(true).widget_instance()
					});
					cells.push(widget);
				}
				cells
			})
			.collect::<Vec<_>>();

		let mut column_names = vec!["", "element"];
		column_names.extend(attribute_keys.iter().map(|s| s.as_str()));
		rows.insert(0, column_headings(&column_names));

		vec![LayoutGroup::table(rows, false)]
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
		let table_tabs = vec![RadioInput::new(table_tab_entries).selected_index(Some(data.vector_table_tab as u32)).widget_instance()];

		let mut table_rows = Vec::new();
		match data.vector_table_tab {
			VectorTableTab::Properties => {
				table_rows.push(column_headings(&["property", "value"]));

				match self.style.fill.clone() {
					Fill::None => table_rows.push(vec![
						TextLabel::new("Fill").narrow(true).widget_instance(),
						ColorInput::new(FillChoice::None).disabled(true).menu_direction(Some(MenuDirection::Top)).narrow(true).widget_instance(),
					]),
					Fill::Solid(color) => table_rows.push(vec![
						TextLabel::new("Fill").narrow(true).widget_instance(),
						ColorInput::new(FillChoice::Solid(color))
							.disabled(true)
							.menu_direction(Some(MenuDirection::Top))
							.narrow(true)
							.widget_instance(),
					]),
					Fill::Gradient(gradient) => {
						table_rows.push(vec![
							TextLabel::new("Fill").narrow(true).widget_instance(),
							ColorInput::new(FillChoice::Gradient(gradient.stops))
								.disabled(true)
								.menu_direction(Some(MenuDirection::Top))
								.narrow(true)
								.widget_instance(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient Type").narrow(true).widget_instance(),
							TextLabel::new(gradient.gradient_type.to_string()).narrow(true).widget_instance(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient Start").narrow(true).widget_instance(),
							TextLabel::new(format_dvec2(gradient.start)).narrow(true).widget_instance(),
						]);
						table_rows.push(vec![
							TextLabel::new("Fill Gradient End").narrow(true).widget_instance(),
							TextLabel::new(format_dvec2(gradient.end)).narrow(true).widget_instance(),
						]);
					}
				}

				if let Some(stroke) = self.style.stroke.clone() {
					let color = if let Some(color) = stroke.color { FillChoice::Solid(color) } else { FillChoice::None };
					table_rows.push(vec![
						TextLabel::new("Stroke").narrow(true).widget_instance(),
						ColorInput::new(color).disabled(true).menu_direction(Some(MenuDirection::Top)).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Weight").narrow(true).widget_instance(),
						TextLabel::new(format!("{} px", stroke.weight)).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Dash Lengths").narrow(true).widget_instance(),
						TextLabel::new(if stroke.dash_lengths.is_empty() {
							"-".to_string()
						} else {
							format!("[{}]", stroke.dash_lengths.iter().map(|x| format!("{x} px")).collect::<Vec<_>>().join(", "))
						})
						.narrow(true)
						.widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Dash Offset").narrow(true).widget_instance(),
						TextLabel::new(format!("{}", stroke.dash_offset)).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Cap").narrow(true).widget_instance(),
						TextLabel::new(stroke.cap.to_string()).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Join").narrow(true).widget_instance(),
						TextLabel::new(stroke.join.to_string()).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Join Miter Limit").narrow(true).widget_instance(),
						TextLabel::new(format!("{}", stroke.join_miter_limit)).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Align").narrow(true).widget_instance(),
						TextLabel::new(stroke.align.to_string()).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Transform").narrow(true).widget_instance(),
						TextLabel::new(format_transform_matrix(stroke.transform)).narrow(true).widget_instance(),
					]);
					table_rows.push(vec![
						TextLabel::new("Stroke Paint Order").narrow(true).widget_instance(),
						TextLabel::new(stroke.paint_order.to_string()).narrow(true).widget_instance(),
					]);
				}

				let colinear = self.colinear_manipulators.iter().map(|[a, b]| format!("[{a} / {b}]")).collect::<Vec<_>>().join(", ");
				let colinear = if colinear.is_empty() { "-".to_string() } else { colinear };
				table_rows.push(vec![
					TextLabel::new("Colinear Handle IDs").narrow(true).widget_instance(),
					TextLabel::new(colinear).narrow(true).widget_instance(),
				]);
			}
			VectorTableTab::Points => {
				table_rows.push(column_headings(&["", "position"]));
				table_rows.extend(self.point_domain.iter().map(|(id, position)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_instance(),
						TextLabel::new(format!("{position}")).narrow(true).widget_instance(),
					]
				}));
			}
			VectorTableTab::Segments => {
				table_rows.push(column_headings(&["", "start_index", "end_index", "handles"]));
				table_rows.extend(self.segment_domain.iter().map(|(id, start, end, handles)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_instance(),
						TextLabel::new(format!("{start}")).narrow(true).widget_instance(),
						TextLabel::new(format!("{end}")).narrow(true).widget_instance(),
						TextLabel::new(format!("{handles:?}")).narrow(true).widget_instance(),
					]
				}));
			}
			VectorTableTab::Regions => {
				table_rows.push(column_headings(&["", "segment_range", "fill"]));
				table_rows.extend(self.region_domain.iter().map(|(id, segment_range, fill)| {
					vec![
						TextLabel::new(format!("{}", id.inner())).narrow(true).widget_instance(),
						TextLabel::new(format!("{segment_range:?}")).narrow(true).widget_instance(),
						TextLabel::new(format!("{}", fill.inner())).narrow(true).widget_instance(),
					]
				}));
			}
		}

		vec![LayoutGroup::row(table_tabs), LayoutGroup::table(table_rows, false)]
	}
}

impl TableRowLayout for Raster<CPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({} x {})", self.width, self.height)
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let raster = self.data();

		if raster.width == 0 || raster.height == 0 {
			let widgets = vec![TextLabel::new("Image has no area").widget_instance()];
			return vec![LayoutGroup::row(widgets)];
		}

		let base64_string = raster.base64_string.clone().unwrap_or_else(|| {
			use base64::Engine;

			let output = raster.to_png();
			let preamble = "data:image/png;base64,";
			let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
			base64_string.push_str(preamble);
			base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
			base64_string
		});

		let widgets = vec![ImageLabel::new(base64_string).widget_instance()];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableRowLayout for Raster<GPU> {
	fn type_name() -> &'static str {
		"Raster"
	}
	fn identifier(&self) -> String {
		format!("Raster ({} x {})", self.data().width(), self.data().height())
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![TextLabel::new("Raster is a texture on the GPU and cannot currently be displayed here").widget_instance()];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableRowLayout for Color {
	fn type_name() -> &'static str {
		"Color"
	}
	fn identifier(&self) -> String {
		format!("Color (#{})", self.to_gamma_srgb().to_rgba_hex_srgb())
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		ColorInput::new(FillChoice::Solid(*self))
			.disabled(true)
			.menu_direction(Some(MenuDirection::Top))
			.narrow(true)
			.widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![self.cell_widget(PathStep::Element(0), _data)];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableRowLayout for GradientStops {
	fn type_name() -> &'static str {
		"Gradient"
	}
	fn identifier(&self) -> String {
		format!("Gradient ({} stops)", self.len())
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		ColorInput::new(FillChoice::Gradient(self.clone()))
			.menu_direction(Some(MenuDirection::Top))
			.disabled(true)
			.narrow(true)
			.widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		let widgets = vec![self.cell_widget(PathStep::Element(0), _data)];
		vec![LayoutGroup::row(widgets)]
	}
}

impl TableRowLayout for f64 {
	fn type_name() -> &'static str {
		"Number (f64)"
	}
	fn identifier(&self) -> String {
		format!("{self}")
	}
	// Cells fall back to the default drill-in button (labeled with the value via `identifier`); the leaf page shows the rich `NumberInput`.
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![
			NumberInput::new(Some(*self)).disabled(true).max_width(220).display_decimal_places(20).widget_instance(),
		])]
	}
}

impl TableRowLayout for u8 {
	fn type_name() -> &'static str {
		"Byte"
	}
	fn identifier(&self) -> String {
		format!("{self:02X}")
	}
	// Cells fall back to the default drill-in button (labeled with the hex value via `identifier`); the leaf page shows the same hex value as a label.
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![TextLabel::new(self.identifier()).widget_instance()])]
	}
}

impl TableRowLayout for u32 {
	fn type_name() -> &'static str {
		"Number (u32)"
	}
	fn identifier(&self) -> String {
		format!("{self}")
	}
	// Cells fall back to the default drill-in button (labeled with the value via `identifier`); the leaf page shows the rich `NumberInput`.
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![
			NumberInput::new(Some(*self as f64)).disabled(true).max_width(220).display_decimal_places(20).widget_instance(),
		])]
	}
}

impl TableRowLayout for u64 {
	fn type_name() -> &'static str {
		"Number (u64)"
	}
	fn identifier(&self) -> String {
		format!("{self}")
	}
	// Cells fall back to the default drill-in button (labeled with the value via `identifier`); the leaf page shows the rich `NumberInput`.
	// TODO: Make this robust for large u64 values that don't fit in f64 (above roughly 2^53). Perhaps using a bigint kind of approach through the widget's data flow.
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![
			NumberInput::new(Some(*self as f64)).disabled(true).max_width(220).display_decimal_places(20).widget_instance(),
		])]
	}
}

impl TableRowLayout for bool {
	fn type_name() -> &'static str {
		"Bool"
	}
	fn identifier(&self) -> String {
		"Bool".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(self.to_string()).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for String {
	fn type_name() -> &'static str {
		"String"
	}
	fn identifier(&self) -> String {
		// Show the first line, and if there are more, indicate that with an ellipsis
		let first_line = self.lines().next().unwrap_or("");
		if self.lines().count() > 1 {
			format!("\"{} …\"", first_line)
		} else {
			format!("\"{}\"", first_line)
		}
	}
	// Cells fall back to the default drill-in button (labeled with the truncated quoted preview via `identifier`); the leaf page shows the full multi-line text in a `TextAreaInput`.
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![TextAreaInput::new(self.to_string()).monospace(true).disabled(true).widget_instance()])]
	}
}

impl TableRowLayout for Option<f64> {
	fn type_name() -> &'static str {
		"Option<f64>"
	}
	fn identifier(&self) -> String {
		"Option<f64>".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(format!("{self:?}")).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for DVec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(format_dvec2(*self)).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for Vec2 {
	fn type_name() -> &'static str {
		"Vec2"
	}
	fn identifier(&self) -> String {
		"Vec2".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(format_dvec2(DVec2::new(self.x as f64, self.y as f64))).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for DAffine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(format_transform_matrix(*self)).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for Affine2 {
	fn type_name() -> &'static str {
		"Transform"
	}
	fn identifier(&self) -> String {
		"Transform".to_string()
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		let matrix = DAffine2::from_cols_array(&self.to_cols_array().map(|x| x as f64));
		TextLabel::new(format_transform_matrix(matrix)).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

impl TableRowLayout for AlphaBlending {
	fn type_name() -> &'static str {
		"AlphaBlending"
	}
	fn identifier(&self) -> String {
		format_alpha_blending(*self)
	}
	fn cell_widget(&self, _target: PathStep, _data: &LayoutData) -> WidgetInstance {
		TextLabel::new(format_alpha_blending(*self)).narrow(true).widget_instance()
	}
	fn element_page(&self, _data: &mut LayoutData) -> Vec<LayoutGroup> {
		vec![LayoutGroup::row(vec![self.cell_widget(PathStep::Element(0), _data)])]
	}
}

/// Resolves the cell/breadcrumb label for a `NodeId` from the root network's metadata, falling back
/// to "Node {id}" if the node isn't present (e.g. an ID that no longer maps to a real node).
fn node_id_display_label(node_id: NodeId, network_interface: &NodeNetworkInterface) -> String {
	let network_path: &[NodeId] = &[];
	if network_interface.node_metadata(&node_id, network_path).is_some() {
		network_interface.display_name(&node_id, network_path)
	} else {
		format!("Node {node_id}")
	}
}

impl TableRowLayout for NodeId {
	fn type_name() -> &'static str {
		"NodeId"
	}
	fn identifier(&self) -> String {
		format!("Node {self}")
	}
	// Override so the breadcrumb uses the same resolved display name as the cell button, instead of the bare-ID fallback `identifier()` returns.
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		data.breadcrumbs.push(node_id_display_label(*self, data.network_interface));
		self.element_page(data)
	}
	// Cell label resolves the node's display name via the network interface (looked up at the root network) so the
	// button reads as the name shown in the Node Graph / Layers panels. Falls back to "Node {id}" if the lookup misses.
	fn cell_widget(&self, target: PathStep, data: &LayoutData) -> WidgetInstance {
		let label = node_id_display_label(*self, data.network_interface);
		TextButton::new(label)
			.on_update(move |_| DataPanelMessage::PushToElementPath { step: target.clone() }.into())
			.narrow(true)
			.widget_instance()
	}
	// The leaf page shows the node's kind, name, lock/visibility toggles, and a "Make Selected" action button.
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		let node_id = *self;
		// Layer NodeIds (e.g. via the `editor:layer` attribute) live at the root network; if the lookup misses we just show the placeholder name.
		let network_path: &[NodeId] = &[];
		let known = data.network_interface.node_metadata(&node_id, network_path).is_some();
		let name = if known {
			data.network_interface.display_name(&node_id, network_path)
		} else {
			"(node not found in root network)".to_string()
		};
		let kind_widget = if known {
			let icon = if data.network_interface.is_layer(&node_id, network_path) { "Layer" } else { "Node" };
			IconLabel::new(icon).widget_instance()
		} else {
			TextLabel::new("-").widget_instance()
		};

		let mut header = vec![kind_widget, Separator::new(SeparatorStyle::Related).widget_instance(), TextLabel::new(name).widget_instance()];

		if known {
			let is_locked = data.network_interface.is_locked(&node_id, network_path);
			let is_visible = data.network_interface.is_visible(&node_id, network_path);

			header.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			header.push(
				IconButton::new(if is_locked { "PadlockLocked" } else { "PadlockUnlocked" }, 24)
					.hover_icon(if is_locked { "PadlockUnlocked" } else { "PadlockLocked" })
					.tooltip_label(if is_locked { "Unlock" } else { "Lock" })
					.on_update(move |_| NodeGraphMessage::ToggleLocked { node_id }.into())
					.widget_instance(),
			);
			header.push(
				IconButton::new(if is_visible { "EyeVisible" } else { "EyeHidden" }, 24)
					.hover_icon(if is_visible { "EyeHide" } else { "EyeShow" })
					.tooltip_label(if is_visible { "Hide" } else { "Show" })
					.on_update(move |_| NodeGraphMessage::ToggleVisibility { node_id }.into())
					.widget_instance(),
			);
		}

		header.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		header.push(
			TextButton::new("Make Selected")
				.tooltip_description("Click to select the node with this ID in the graph.")
				.on_update(move |_| NodeGraphMessage::SelectedNodesSet { nodes: vec![node_id] }.into())
				.widget_instance(),
		);

		vec![LayoutGroup::row(header)]
	}
}

impl TableRowLayout for Option<NodeId> {
	fn type_name() -> &'static str {
		"NodeId"
	}
	fn identifier(&self) -> String {
		match self {
			Some(node_id) => format!("Node {}", node_id),
			None => "-".to_string(),
		}
	}
	// Cells defer to `NodeId`'s named cell button for `Some` (so the label reads as the node's display name),
	// or render a plain "-" label for `None`. The leaf page likewise defers to `NodeId` for `Some`.
	fn cell_widget(&self, target: PathStep, data: &LayoutData) -> WidgetInstance {
		match self {
			Some(node_id) => node_id.cell_widget(target, data),
			None => TextLabel::new("-").narrow(true).widget_instance(),
		}
	}
	// Defer to `NodeId`'s breadcrumb for `Some` so it stays in sync with the cell label; `None` shows just "-".
	fn layout_with_breadcrumb(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Some(node_id) => node_id.layout_with_breadcrumb(data),
			None => {
				data.breadcrumbs.push("-".to_string());
				self.element_page(data)
			}
		}
	}
	fn element_page(&self, data: &mut LayoutData) -> Vec<LayoutGroup> {
		match self {
			Some(node_id) => node_id.element_page(data),
			None => vec![LayoutGroup::row(vec![TextLabel::new("-").widget_instance()])],
		}
	}
}

/// Invokes another macro with the full list of `TableRowLayout`-implementing types whose values may appear
/// as attribute cell values. Both the cell-rendering and drilldown-navigation dispatchers iterate this list,
/// so adding a new attribute-displayable type is a single edit here.
macro_rules! known_table_row_types {
	($apply:ident) => {
		$apply!(
			Table<Artboard>,
			Table<Graphic>,
			Table<Vector>,
			Table<Raster<CPU>>,
			Table<Raster<GPU>>,
			Table<Color>,
			Table<GradientStops>,
			Table<String>,
			Table<NodeId>,
			Table<f64>,
			Table<u8>,
			GradientStops,
			Color,
			NodeId,
			Option<NodeId>,
			AlphaBlending,
			DAffine2,
			DVec2,
			Affine2,
			Vec2,
			Option<f64>,
			f64,
			u8,
			u32,
			u64,
			bool,
			String,
			Vector,
			Raster<CPU>,
			Raster<GPU>,
			Artboard,
			Graphic,
		);
	};
}

/// Type-dispatched widget for displaying an attribute cell in a `Table<T>` row.
/// Delegates to [`TableRowLayout::cell_widget`] so the same widget code is shared between
/// element-column rendering and attribute-column rendering. Returns `None` for unrecognized types so the
/// caller can fall back to a debug-formatted [`TextLabel`].
fn dispatch_cell_widget(any: &dyn Any, target: PathStep, data: &LayoutData) -> Option<WidgetInstance> {
	macro_rules! check {
		( $($ty:ty),* $(,)? ) => {
			$(
				if let Some(value) = any.downcast_ref::<$ty>() {
					return Some(value.cell_widget(target, data));
				}
			)*
		};
	}
	known_table_row_types!(check);
	None
}

/// Type-dispatched recursion into an attribute value for the data panel breadcrumb navigation.
/// Mirrors [`dispatch_cell_widget`] but routes to [`TableRowLayout::layout_with_breadcrumb`].
/// Returns `None` for unrecognized types.
fn drilldown_attribute_layout(any: &dyn Any, data: &mut LayoutData) -> Option<Vec<LayoutGroup>> {
	macro_rules! check {
		( $($ty:ty),* $(,)? ) => {
			$(
				if let Some(value) = any.downcast_ref::<$ty>() {
					return Some(value.layout_with_breadcrumb(data));
				}
			)*
		};
	}
	known_table_row_types!(check);
	None
}

fn format_transform_matrix(transform: DAffine2) -> String {
	let (scale, angle, translation) = if transform.matrix2.determinant().abs() <= f64::EPSILON {
		let [col_0, col_1] = transform.matrix2.to_cols_array_2d().map(|[x, y]| DVec2::new(x, y));

		let scale = DVec2::new(col_0.length(), col_1.length());

		let rotation = if scale.x > f64::EPSILON {
			col_0.y.atan2(col_0.x)
		} else if scale.y > f64::EPSILON {
			col_1.y.atan2(col_1.x) - std::f64::consts::FRAC_PI_2
		} else {
			0.
		};

		(scale, rotation, transform.translation)
	} else {
		transform.to_scale_angle_translation()
	};
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

fn format_alpha_blending(value: AlphaBlending) -> String {
	let round = |x: f32| (x * 1e3).round() / 1e3;
	format!(
		"Blend Mode: {} — Opacity: {}% — Fill: {}% — Clip: {}",
		value.blend_mode,
		round(value.opacity * 100.),
		round(value.fill * 100.),
		if value.clip { "Yes" } else { "No" }
	)
}
