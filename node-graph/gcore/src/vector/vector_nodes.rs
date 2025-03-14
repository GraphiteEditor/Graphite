use super::misc::CentroidType;
use super::style::{Fill, Gradient, GradientStops, Stroke};
use super::{PointId, SegmentDomain, SegmentId, StrokeId, VectorData, VectorDataTable};
use crate::instances::{InstanceMut, Instances};
use crate::registry::types::{Angle, Fraction, IntegerCount, Length, SeedValue};
use crate::renderer::GraphicElementRendered;
use crate::transform::{Footprint, Transform, TransformMut};
use crate::vector::PointDomain;
use crate::vector::style::LineJoin;
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, GraphicElement, GraphicGroupTable, OwnedContextImpl};
use bezier_rs::{Cap, Join, Subpath, SubpathTValue, TValue};
use glam::{DAffine2, DVec2};
use rand::{Rng, SeedableRng};

/// Implemented for types that can be converted to an iterator of vector data.
/// Used for the fill and stroke node so they can be used on VectorData or GraphicGroup
trait VectorDataTableIterMut {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<VectorData>>;
}

impl VectorDataTableIterMut for GraphicGroupTable {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<VectorData>> {
		// Grab only the direct children
		self.instances_mut()
			.filter_map(|element| element.instance.as_vector_data_mut())
			.flat_map(move |vector_data| vector_data.instances_mut())
	}
}

impl VectorDataTableIterMut for VectorDataTable {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<VectorData>> {
		self.instances_mut()
	}
}

#[node_macro::node(category("Vector: Style"), path(graphene_core::vector))]
async fn assign_colors<T>(
	_: impl Ctx,
	#[implementations(GraphicGroupTable, VectorDataTable)]
	#[widget(ParsedWidgetOverride::Hidden)]
	/// The vector elements, or group of vector elements, to apply the fill and/or stroke style to.
	mut vector_group: T,
	#[default(true)]
	/// Whether to style the fill.
	fill: bool,
	/// Whether to style the stroke.
	stroke: bool,
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_gradient")]
	/// The range of colors to select from.
	gradient: GradientStops,
	/// Whether to reverse the gradient.
	reverse: bool,
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_randomize")]
	/// Whether to randomize the color selection for each element from throughout the gradient.
	randomize: bool,
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_seed")]
	/// The seed used for randomization.
	seed: SeedValue,
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_repeat_every")]
	/// The number of elements to span across the gradient before repeating. A 0 value will span the entire gradient once.
	repeat_every: u32,
) -> T
where
	T: VectorDataTableIterMut + 'n + Send,
{
	let length = vector_group.vector_iter_mut().count();
	let gradient = if reverse { gradient.reversed() } else { gradient };

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

	for (i, vector_data) in vector_group.vector_iter_mut().enumerate() {
		let factor = match randomize {
			true => rng.random::<f64>(),
			false => match repeat_every {
				0 => i as f64 / (length - 1).max(1) as f64,
				1 => 0.,
				_ => i as f64 % repeat_every as f64 / (repeat_every - 1) as f64,
			},
		};

		let color = gradient.evalute(factor);

		if fill {
			vector_data.instance.style.set_fill(Fill::Solid(color));
		}
		if stroke {
			if let Some(stroke) = vector_data.instance.style.stroke().and_then(|stroke| stroke.with_color(&Some(color))) {
				vector_data.instance.style.set_stroke(stroke);
			}
		}
	}

	vector_group
}

#[node_macro::node(category("Vector: Style"), path(graphene_core::vector), properties("fill_properties"))]
async fn fill<F: Into<Fill> + 'n + Send, V>(
	_: impl Ctx,
	#[implementations(
		VectorDataTable,
		VectorDataTable,
		VectorDataTable,
		VectorDataTable,
		GraphicGroupTable,
		GraphicGroupTable,
		GraphicGroupTable,
		GraphicGroupTable
	)]
	/// The vector elements, or group of vector elements, to apply the fill to.
	mut vector_data: V,
	#[implementations(
		Fill,
		Option<Color>,
		Color,
		Gradient,
		Fill,
		Option<Color>,
		Color,
		Gradient,
	)]
	#[default(Color::BLACK)]
	/// The fill to paint the path with.
	fill: F,
	_backup_color: Option<Color>,
	_backup_gradient: Gradient,
) -> V
where
	V: VectorDataTableIterMut + 'n + Send,
{
	let fill: Fill = fill.into();
	for vector in vector_data.vector_iter_mut() {
		let mut fill = fill.clone();
		if let Fill::Gradient(gradient) = &mut fill {
			gradient.transform *= *vector.transform;
		}
		vector.instance.style.set_fill(fill);
	}

	vector_data
}

/// Applies a stroke style to the vector data contained in the input.
#[node_macro::node(category("Vector: Style"), path(graphene_core::vector), properties("stroke_properties"))]
async fn stroke<C: Into<Option<Color>> + 'n + Send, V>(
	_: impl Ctx,
	#[implementations(VectorDataTable, VectorDataTable, GraphicGroupTable, GraphicGroupTable)]
	/// The vector elements, or group of vector elements, to apply the stroke to.
	mut vector_data: Instances<V>,
	#[implementations(
		Option<Color>,
		Color,
		Option<Color>,
		Color,
	)]
	#[default(Color::BLACK)]
	/// The stroke color.
	color: C,
	#[default(2.)]
	/// The stroke weight.
	weight: f64,
	/// The stroke dash lengths. Each length forms a distance in a pattern where the first length is a dash, the second is a gap, and so on. If the list is an odd length, the pattern repeats with solid-gap roles reversed.
	dash_lengths: Vec<f64>,
	/// The offset distance from the starting point of the dash pattern.
	dash_offset: f64,
	/// The shape of the stroke at open endpoints.
	line_cap: crate::vector::style::LineCap,
	/// The curvature of the bent stroke at sharp corners.
	line_join: LineJoin,
	#[default(4.)]
	/// The threshold for when a miter-joined stroke is converted to a bevel-joined stroke when a sharp angle becomes pointier than this ratio.
	miter_limit: f64,
) -> Instances<V>
where
	Instances<V>: VectorDataTableIterMut + 'n + Send,
{
	let stroke = Stroke {
		color: color.into(),
		weight,
		dash_lengths,
		dash_offset,
		line_cap,
		line_join,
		line_join_miter_limit: miter_limit,
		transform: DAffine2::IDENTITY,
	};
	for vector in vector_data.vector_iter_mut() {
		let mut stroke = stroke.clone();
		stroke.transform *= *vector.transform;
		vector.instance.style.set_stroke(stroke);
	}

	vector_data
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn repeat<I: 'n + Send>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(VectorDataTable, GraphicGroupTable)] instance: Instances<I>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: DVec2,
	angle: Angle,
	#[default(4)] instances: IntegerCount,
) -> GraphicGroupTable
where
	Instances<I>: GraphicElementRendered,
{
	let angle = angle.to_radians();
	let instances = instances.max(1);
	let total = (instances - 1) as f64;

	let mut result_table = GraphicGroupTable::default();

	let Some(bounding_box) = instance.bounding_box(DAffine2::IDENTITY) else { return result_table };

	let center = (bounding_box[0] + bounding_box[1]) / 2.;

	for index in 0..instances {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let modification = DAffine2::from_translation(center) * DAffine2::from_angle(angle) * DAffine2::from_translation(translation) * DAffine2::from_translation(-center);

		let mut new_graphic_element = instance.to_graphic_element().clone();
		new_graphic_element.new_ids_from_hash(Some(crate::uuid::NodeId(index as u64)));

		let new_instance = result_table.push(new_graphic_element);
		*new_instance.transform = modification;
	}

	result_table
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn circular_repeat<I: 'n + Send>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(VectorDataTable, GraphicGroupTable)] instance: Instances<I>,
	angle_offset: Angle,
	#[default(5)] radius: f64,
	#[default(5)] instances: IntegerCount,
) -> GraphicGroupTable
where
	Instances<I>: GraphicElementRendered,
{
	let instances = instances.max(1);

	let mut result_table = GraphicGroupTable::default();

	let Some(bounding_box) = instance.bounding_box(DAffine2::IDENTITY) else { return result_table };

	let center = (bounding_box[0] + bounding_box[1]) / 2.;
	let base_transform = DVec2::new(0., radius) - center;

	for index in 0..instances {
		let rotation = DAffine2::from_angle((std::f64::consts::TAU / instances as f64) * index as f64 + angle_offset.to_radians());
		let modification = DAffine2::from_translation(center) * rotation * DAffine2::from_translation(base_transform);

		let mut new_graphic_element = instance.to_graphic_element().clone();
		new_graphic_element.new_ids_from_hash(Some(crate::uuid::NodeId(index as u64)));

		let new_instance = result_table.push(new_graphic_element);
		*new_instance.transform = modification;
	}

	result_table
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn copy_to_points<I: 'n + Send>(
	_: impl Ctx,
	points: VectorDataTable,
	#[expose]
	#[implementations(VectorDataTable, GraphicGroupTable)]
	instance: Instances<I>,
	#[default(1)] random_scale_min: f64,
	#[default(1)] random_scale_max: f64,
	random_scale_bias: f64,
	random_scale_seed: SeedValue,
	random_rotation: Angle,
	random_rotation_seed: SeedValue,
) -> GraphicGroupTable
where
	Instances<I>: GraphicElementRendered,
{
	let points_transform = points.transform();
	let points_list = points.instances().flat_map(|element| element.instance.point_domain.positions());

	let random_scale_difference = random_scale_max - random_scale_min;

	let instance_bounding_box = instance.bounding_box(DAffine2::IDENTITY).unwrap_or_default();
	let instance_center = -0.5 * (instance_bounding_box[0] + instance_bounding_box[1]);

	let mut scale_rng = rand::rngs::StdRng::seed_from_u64(random_scale_seed.into());
	let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(random_rotation_seed.into());

	let do_scale = random_scale_difference.abs() > 1e-6;
	let do_rotation = random_rotation.abs() > 1e-6;

	let mut result_table = GraphicGroupTable::default();

	for (index, &point) in points_list.into_iter().enumerate() {
		let center_transform = DAffine2::from_translation(instance_center);

		let translation = points_transform.transform_point2(point);

		let rotation = if do_rotation {
			let degrees = (rotation_rng.random::<f64>() - 0.5) * random_rotation;
			degrees / 360. * std::f64::consts::TAU
		} else {
			0.
		};

		let scale = if do_scale {
			if random_scale_bias.abs() < 1e-6 {
				// Linear
				random_scale_min + scale_rng.random::<f64>() * random_scale_difference
			} else {
				// Weighted (see <https://www.desmos.com/calculator/gmavd3m9bd>)
				let horizontal_scale_factor = 1. - 2_f64.powf(random_scale_bias);
				let scale_factor = (1. - scale_rng.random::<f64>() * horizontal_scale_factor).log2() / random_scale_bias;
				random_scale_min + scale_factor * random_scale_difference
			}
		} else {
			random_scale_min
		};

		let mut new_graphic_element = instance.to_graphic_element().clone();
		new_graphic_element.new_ids_from_hash(Some(crate::uuid::NodeId(index as u64)));

		let new_instance = result_table.push(new_graphic_element);
		*new_instance.transform = DAffine2::from_scale_angle_translation(DVec2::splat(scale), rotation, translation) * center_transform;
	}

	result_table
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn bounding_box(_: impl Ctx, vector_data: VectorDataTable) -> VectorDataTable {
	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	let bounding_box = vector_data.bounding_box_with_transform(vector_data_transform).unwrap();
	let mut result = VectorData::from_subpath(Subpath::new_rect(bounding_box[0], bounding_box[1]));
	result.style = vector_data.style.clone();
	result.style.set_stroke_transform(DAffine2::IDENTITY);

	VectorDataTable::new(result)
}

#[node_macro::node(category("Vector"), path(graphene_core::vector), properties("offset_path_properties"))]
async fn offset_path(_: impl Ctx, vector_data: VectorDataTable, distance: f64, line_join: LineJoin, #[default(4.)] miter_limit: f64) -> VectorDataTable {
	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	let subpaths = vector_data.stroke_bezier_paths();
	let mut result = VectorData::empty();
	result.style = vector_data.style.clone();
	result.style.set_stroke_transform(DAffine2::IDENTITY);

	// Perform operation on all subpaths in this shape.
	for mut subpath in subpaths {
		subpath.apply_transform(vector_data_transform);

		// Taking the existing stroke data and passing it to Bezier-rs to generate new paths.
		let subpath_out = subpath.offset(
			-distance,
			match line_join {
				LineJoin::Miter => Join::Miter(Some(miter_limit)),
				LineJoin::Bevel => Join::Bevel,
				LineJoin::Round => Join::Round,
			},
		);

		// One closed subpath, open path.
		result.append_subpath(subpath_out, false);
	}

	VectorDataTable::new(result)
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn solidify_stroke(_: impl Ctx, vector_data: VectorDataTable) -> VectorDataTable {
	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	let style = &vector_data.style;

	let subpaths = vector_data.stroke_bezier_paths();
	let mut result = VectorData::empty();

	// Perform operation on all subpaths in this shape.
	for mut subpath in subpaths {
		let stroke = style.stroke().unwrap();
		subpath.apply_transform(vector_data_transform);

		// Taking the existing stroke data and passing it to Bezier-rs to generate new paths.
		let subpath_out = subpath.outline(
			stroke.weight / 2., // Diameter to radius.
			match stroke.line_join {
				LineJoin::Miter => Join::Miter(Some(stroke.line_join_miter_limit)),
				LineJoin::Bevel => Join::Bevel,
				LineJoin::Round => Join::Round,
			},
			match stroke.line_cap {
				crate::vector::style::LineCap::Butt => Cap::Butt,
				crate::vector::style::LineCap::Round => Cap::Round,
				crate::vector::style::LineCap::Square => Cap::Square,
			},
		);

		// This is where we determine whether we have a closed or open path. Ex: Oval vs line segment.
		if subpath_out.1.is_some() {
			// Two closed subpaths, closed shape. Add both subpaths.
			result.append_subpath(subpath_out.0, false);
			result.append_subpath(subpath_out.1.unwrap(), false);
		} else {
			// One closed subpath, open path.
			result.append_subpath(subpath_out.0, false);
		}
	}

	// We set our fill to our stroke's color, then clear our stroke.
	if let Some(stroke) = vector_data.style.stroke() {
		result.style.set_fill(Fill::solid_or_none(stroke.color));
		result.style.set_stroke(Stroke::default());
	}

	VectorDataTable::new(result)
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn flatten_vector_elements(_: impl Ctx, graphic_group_input: GraphicGroupTable) -> VectorDataTable {
	// A node based solution to support passing through vector data could be a network node with a cache node connected to
	// a flatten vector elements connected to an if else node, another connection from the cache directly
	// To the if else node, and another connection from the cache to a matches type node connected to the if else node.
	fn flatten_group(graphic_group_table: &GraphicGroupTable, output: &mut InstanceMut<VectorData>) {
		for current_element in graphic_group_table.instances() {
			match current_element.instance {
				GraphicElement::VectorData(vector_data_table) => {
					// Loop through every row of the VectorDataTable and concatenate each instance's subpath into the output VectorData instance.
					for vector_data_instance in vector_data_table.instances() {
						let other = vector_data_instance.instance;
						let transform = *current_element.transform * *vector_data_instance.transform;
						let node_id = current_element.source_node_id.map(|node_id| node_id.0).unwrap_or_default();
						output.instance.concat(other, transform, node_id);

						// Use the last encountered style as the output style
						output.instance.style = vector_data_instance.instance.style.clone();
					}
				}
				GraphicElement::GraphicGroup(graphic_group) => {
					let mut graphic_group = graphic_group.clone();
					for instance in graphic_group.instances_mut() {
						*instance.transform = *current_element.transform * *instance.transform;
					}

					flatten_group(&graphic_group, output);
				}
				_ => {}
			}
		}
	}

	let mut output_table = VectorDataTable::default();
	let Some(mut output) = output_table.instances_mut().next() else { return output_table };

	flatten_group(&graphic_group_input, &mut output);

	output_table
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn sample_points(_: impl Ctx, vector_data: VectorDataTable, spacing: f64, start_offset: f64, stop_offset: f64, adaptive_spacing: bool, subpath_segment_lengths: Vec<f64>) -> VectorDataTable {
	// Limit the smallest spacing to something sensible to avoid freezing the application.
	let spacing = spacing.max(0.01);

	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	// Create an iterator over the bezier segments with enumeration and peeking capability.
	let mut bezier = vector_data.segment_bezier_iter().enumerate().peekable();

	// Initialize the result VectorData with the same transformation as the input.
	let mut result = VectorDataTable::default();
	*result.transform_mut() = vector_data_transform;

	// Iterate over each segment in the bezier iterator.
	while let Some((index, (segment_id, _, start_point_index, mut last_end))) = bezier.next() {
		// Record the start point index of the subpath.
		let subpath_start_point_index = start_point_index;

		// Collect connected segments that form a continuous path.
		let mut lengths = vec![(segment_id, subpath_segment_lengths.get(index).copied().unwrap_or_default())];

		// Continue collecting segments as long as they are connected end-to-start.
		while let Some(&seg) = bezier.peek() {
			let (_, (_, _, ref start, _)) = seg;
			if *start == last_end {
				// Consume the next element since it continues the path.
				let (index, (next_segment_id, _, _, end)) = bezier.next().unwrap();
				last_end = end;
				lengths.push((next_segment_id, subpath_segment_lengths.get(index).copied().unwrap_or_default()));
			} else {
				// The next segment does not continue the path.
				break;
			}
		}

		// Determine if the subpath is closed.
		let subpath_is_closed = last_end == subpath_start_point_index;

		// Calculate the total length of the collected segments.
		let total_length: f64 = lengths.iter().map(|(_, len)| *len).sum();

		// Adjust the usable length by subtracting start and stop offsets.
		let mut used_length = total_length - start_offset - stop_offset;
		if used_length <= 0. {
			continue;
		}

		// Determine the number of points to generate along the path.
		let count = if adaptive_spacing {
			// Calculate point count to evenly distribute points while covering the entire path.
			// With adaptive spacing, we widen or narrow the points as necessary to ensure the last point is always at the end of the path.
			(used_length / spacing).round()
		} else {
			// Calculate point count based on exact spacing, which may not cover the entire path.

			// Without adaptive spacing, we just evenly space the points at the exact specified spacing, usually falling short before the end of the path.
			let c = (used_length / spacing + f64::EPSILON).floor();
			used_length -= used_length % spacing;
			c
		};

		// Skip if there are no points to generate.
		if count < 1. {
			continue;
		}

		// Initialize a vector to store indices of generated points.
		let mut point_indices = Vec::new();

		// Generate points along the path based on calculated intervals.
		let max_c = if subpath_is_closed { count as usize - 1 } else { count as usize };
		for c in 0..=max_c {
			let fraction = c as f64 / count;
			let total_distance = fraction * used_length + start_offset;

			// Find the segment corresponding to the current total_distance.
			let (mut current_segment_id, mut length) = lengths[0];
			let mut total_length_before = 0.;
			for &(next_segment_id, next_length) in lengths.iter().skip(1) {
				if total_length_before + length > total_distance {
					break;
				}

				total_length_before += length;
				current_segment_id = next_segment_id;
				length = next_length;
			}

			// Retrieve the segment and apply transformation.
			let Some(segment) = vector_data.segment_from_id(current_segment_id) else { continue };
			let segment = segment.apply_transformation(|point| vector_data_transform.transform_point2(point));

			// Calculate the position on the segment.
			let parametric_t = segment.euclidean_to_parametric_with_total_length((total_distance - total_length_before) / length, 0.001, length);
			let point = segment.evaluate(TValue::Parametric(parametric_t));

			// Generate a new PointId and add the point to result.point_domain.
			let point_id = PointId::generate();
			result.one_instance_mut().instance.point_domain.push(point_id, vector_data_transform.inverse().transform_point2(point));

			// Store the index of the point.
			let point_index = result.one_instance_mut().instance.point_domain.ids().len() - 1;
			point_indices.push(point_index);
		}

		// After generating points, create segments between consecutive points.
		for window in point_indices.windows(2) {
			if let [start_index, end_index] = *window {
				// Generate a new SegmentId.
				let segment_id = SegmentId::generate();

				// Use BezierHandles::Linear for linear segments.
				let handles = bezier_rs::BezierHandles::Linear;

				// Generate a new StrokeId.
				let stroke_id = StrokeId::generate();

				// Add the segment to result.segment_domain.
				result.one_instance_mut().instance.segment_domain.push(segment_id, start_index, end_index, handles, stroke_id);
			}
		}

		// If the subpath is closed, add a closing segment connecting the last point to the first point.
		if subpath_is_closed {
			if let (Some(&first_index), Some(&last_index)) = (point_indices.first(), point_indices.last()) {
				// Generate a new SegmentId.
				let segment_id = SegmentId::generate();

				// Use BezierHandles::Linear for linear segments.
				let handles = bezier_rs::BezierHandles::Linear;

				// Generate a new StrokeId.
				let stroke_id = StrokeId::generate();

				// Add the closing segment to result.segment_domain.
				result.one_instance_mut().instance.segment_domain.push(segment_id, last_index, first_index, handles, stroke_id);
			}
		}
	}

	// Transfer the style from the input vector data to the result.
	result.one_instance_mut().instance.style = vector_data.style.clone();
	result.one_instance_mut().instance.style.set_stroke_transform(vector_data_transform);

	// Return the resulting vector data with newly generated points and segments.
	result
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn poisson_disk_points(
	_: impl Ctx,
	vector_data: VectorDataTable,
	#[default(10.)]
	#[min(0.01)]
	separation_disk_diameter: f64,
	seed: SeedValue,
) -> VectorDataTable {
	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());
	let mut result = VectorData::empty();

	if separation_disk_diameter <= 0.01 {
		return VectorDataTable::new(result);
	}

	for mut subpath in vector_data.stroke_bezier_paths() {
		if subpath.manipulator_groups().len() < 3 {
			continue;
		}

		subpath.apply_transform(vector_data_transform);

		let mut previous_point_index: Option<usize> = None;

		for point in subpath.poisson_disk_points(separation_disk_diameter, || rng.random::<f64>()) {
			let point_id = PointId::generate();
			result.point_domain.push(point_id, point);

			// Get the index of the newly added point.
			let point_index = result.point_domain.ids().len() - 1;

			// If there is a previous point, connect it with the current point by adding a segment.
			if let Some(prev_point_index) = previous_point_index {
				let segment_id = SegmentId::generate();
				result.segment_domain.push(segment_id, prev_point_index, point_index, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
			}

			previous_point_index = Some(point_index);
		}
	}

	// Transfer the style from the input vector data to the result.
	result.style = vector_data.style.clone();
	result.style.set_stroke_transform(DAffine2::IDENTITY);

	VectorDataTable::new(result)
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn subpath_segment_lengths(_: impl Ctx, vector_data: VectorDataTable) -> Vec<f64> {
	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	vector_data
		.segment_bezier_iter()
		.map(|(_id, bezier, _, _)| bezier.apply_transformation(|point| vector_data_transform.transform_point2(point)).length(None))
		.collect()
}

#[node_macro::node(name("Spline"), category("Vector"), path(graphene_core::vector))]
async fn spline(_: impl Ctx, mut vector_data: VectorDataTable) -> VectorDataTable {
	let original_transform = vector_data.transform();
	let vector_data = vector_data.one_instance_mut().instance;

	// Exit early if there are no points to generate splines from.
	if vector_data.point_domain.positions().is_empty() {
		return VectorDataTable::new(vector_data.clone());
	}

	let mut segment_domain = SegmentDomain::default();
	for subpath in vector_data.stroke_bezier_paths() {
		let positions = subpath.manipulator_groups().iter().map(|group| group.anchor).collect::<Vec<_>>();
		let closed = subpath.closed() && positions.len() > 2;

		// Compute control point handles for Bezier spline.
		let first_handles = if closed {
			bezier_rs::solve_spline_first_handle_closed(&positions)
		} else {
			bezier_rs::solve_spline_first_handle_open(&positions)
		};

		let stroke_id = StrokeId::ZERO;

		// Create segments with computed Bezier handles and add them to vector data.
		for i in 0..(positions.len() - if closed { 0 } else { 1 }) {
			let next_index = (i + 1) % positions.len();

			let start_index = vector_data.point_domain.resolve_id(subpath.manipulator_groups()[i].id).unwrap();
			let end_index = vector_data.point_domain.resolve_id(subpath.manipulator_groups()[next_index].id).unwrap();

			let handle_start = first_handles[i];
			let handle_end = positions[next_index] * 2. - first_handles[next_index];
			let handles = bezier_rs::BezierHandles::Cubic { handle_start, handle_end };

			segment_domain.push(SegmentId::generate(), start_index, end_index, handles, stroke_id);
		}
	}
	vector_data.segment_domain = segment_domain;

	let mut result = VectorDataTable::new(vector_data.clone());
	*result.transform_mut() = original_transform;
	result
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn jitter_points(_: impl Ctx, vector_data: VectorDataTable, #[default(5.)] amount: f64, seed: SeedValue) -> VectorDataTable {
	let vector_data_transform = vector_data.transform();
	let mut vector_data = vector_data.one_instance().instance.clone();

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

	let deltas = (0..vector_data.point_domain.positions().len())
		.map(|_| {
			let angle = rng.random::<f64>() * std::f64::consts::TAU;
			DVec2::from_angle(angle) * rng.random::<f64>() * amount
		})
		.collect::<Vec<_>>();
	let mut already_applied = vec![false; vector_data.point_domain.positions().len()];

	for (handles, start, end) in vector_data.segment_domain.handles_and_points_mut() {
		let start_delta = deltas[*start];
		let end_delta = deltas[*end];

		if !already_applied[*start] {
			let start_position = vector_data.point_domain.positions()[*start];
			let start_position = vector_data_transform.transform_point2(start_position);
			vector_data.point_domain.set_position(*start, start_position + start_delta);
			already_applied[*start] = true;
		}
		if !already_applied[*end] {
			let end_position = vector_data.point_domain.positions()[*end];
			let end_position = vector_data_transform.transform_point2(end_position);
			vector_data.point_domain.set_position(*end, end_position + end_delta);
			already_applied[*end] = true;
		}

		match handles {
			bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
				*handle_start = vector_data_transform.transform_point2(*handle_start) + start_delta;
				*handle_end = vector_data_transform.transform_point2(*handle_end) + end_delta;
			}
			bezier_rs::BezierHandles::Quadratic { handle } => {
				*handle = vector_data_transform.transform_point2(*handle) + (start_delta + end_delta) / 2.;
			}
			bezier_rs::BezierHandles::Linear => {}
		}
	}

	vector_data.style.set_stroke_transform(DAffine2::IDENTITY);

	VectorDataTable::new(vector_data)
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn morph(
	_: impl Ctx,
	source: VectorDataTable,
	#[expose] target: VectorDataTable,
	#[range((0., 1.))]
	#[default(0.5)]
	time: Fraction,
	#[min(0.)] start_index: IntegerCount,
) -> VectorDataTable {
	let time = time.clamp(0., 1.);

	let source_alpha_blending = source.one_instance().alpha_blending;
	let target_alpha_blending = target.one_instance().alpha_blending;

	let source_transform = source.transform();
	let target_transform = target.transform();

	let source = source.one_instance().instance;
	let target = target.one_instance().instance;

	let mut result = VectorDataTable::default();

	// Lerp styles
	*result.one_instance_mut().alpha_blending = if time < 0.5 { *source_alpha_blending } else { *target_alpha_blending };
	result.one_instance_mut().instance.style = source.style.lerp(&target.style, time);

	let mut source_paths = source.stroke_bezier_paths();
	let mut target_paths = target.stroke_bezier_paths();
	for (mut source_path, mut target_path) in (&mut source_paths).zip(&mut target_paths) {
		// Deal with mismatched transforms
		source_path.apply_transform(source_transform);
		target_path.apply_transform(target_transform);

		// Deal with mismatched start index
		for _ in 0..start_index {
			let first = target_path.remove_manipulator_group(0);
			target_path.push_manipulator_group(first);
		}

		// Deal with mismatched closed state
		if source_path.closed() && !target_path.closed() {
			source_path.set_closed(false);
			source_path.push_manipulator_group(source_path.manipulator_groups()[0].flip());
		}
		if !source_path.closed() && target_path.closed() {
			target_path.set_closed(false);
			target_path.push_manipulator_group(target_path.manipulator_groups()[0].flip());
		}

		// Mismatched subpath items
		'outer: loop {
			for segment_index in (0..(source_path.len() - 1)).rev() {
				if target_path.len() <= source_path.len() {
					break 'outer;
				}
				source_path.insert(SubpathTValue::Parametric { segment_index, t: 0.5 })
			}
		}
		'outer: loop {
			for segment_index in (0..(target_path.len() - 1)).rev() {
				if source_path.len() <= target_path.len() {
					break 'outer;
				}
				target_path.insert(SubpathTValue::Parametric { segment_index, t: 0.5 })
			}
		}

		// Lerp points
		for (manipulator, target) in source_path.manipulator_groups_mut().iter_mut().zip(target_path.manipulator_groups()) {
			manipulator.in_handle = Some(manipulator.in_handle.unwrap_or(manipulator.anchor).lerp(target.in_handle.unwrap_or(target.anchor), time));
			manipulator.out_handle = Some(manipulator.out_handle.unwrap_or(manipulator.anchor).lerp(target.out_handle.unwrap_or(target.anchor), time));
			manipulator.anchor = manipulator.anchor.lerp(target.anchor, time);
		}

		result.one_instance_mut().instance.append_subpath(source_path, true);
	}

	// Mismatched subpath count
	for mut source_path in source_paths {
		source_path.apply_transform(source_transform);
		let end = source_path.manipulator_groups().first().map(|group| group.anchor).unwrap_or_default();
		for group in source_path.manipulator_groups_mut() {
			group.anchor = group.anchor.lerp(end, time);
			group.in_handle = group.in_handle.map(|handle| handle.lerp(end, time));
			group.out_handle = group.in_handle.map(|handle| handle.lerp(end, time));
		}
	}
	for mut target_path in target_paths {
		target_path.apply_transform(target_transform);
		let start = target_path.manipulator_groups().first().map(|group| group.anchor).unwrap_or_default();
		for group in target_path.manipulator_groups_mut() {
			group.anchor = start.lerp(group.anchor, time);
			group.in_handle = group.in_handle.map(|handle| start.lerp(handle, time));
			group.out_handle = group.in_handle.map(|handle| start.lerp(handle, time));
		}
	}

	result
}

fn bevel_algorithm(mut vector_data: VectorData, vector_data_transform: DAffine2, distance: f64) -> VectorData {
	// Splits a bézier curve based on a distance measurement
	fn split_distance(bezier: bezier_rs::Bezier, distance: f64, length: f64) -> bezier_rs::Bezier {
		const EUCLIDEAN_ERROR: f64 = 0.001;
		let parametric = bezier.euclidean_to_parametric_with_total_length((distance / length).clamp(0., 1.), EUCLIDEAN_ERROR, length);
		bezier.split(bezier_rs::TValue::Parametric(parametric))[1]
	}

	/// Produces a list that corresponds with the point ID. The value is how many segments are connected.
	fn segments_connected_count(vector_data: &VectorData) -> Vec<usize> {
		// Count the number of segments connecting to each point.
		let mut segments_connected_count = vec![0; vector_data.point_domain.ids().len()];
		for &point_index in vector_data.segment_domain.start_point().iter().chain(vector_data.segment_domain.end_point()) {
			segments_connected_count[point_index] += 1;
		}

		// Zero out points without exactly two connectors. These are ignored
		for count in &mut segments_connected_count {
			if *count != 2 {
				*count = 0;
			}
		}
		segments_connected_count
	}

	/// Updates the index so that it points at a point with the position. If nobody else will look at the index, the original point is updated. Otherwise a new point is created.
	fn create_or_modify_point(point_domain: &mut PointDomain, segments_connected_count: &mut [usize], pos: DVec2, index: &mut usize, next_id: &mut PointId, new_segments: &mut Vec<[usize; 2]>) {
		segments_connected_count[*index] -= 1;
		if segments_connected_count[*index] == 0 {
			// If nobody else is going to look at this point, we're alright to modify it
			point_domain.set_position(*index, pos);
		} else {
			let new_index = point_domain.ids().len();
			let original_index = *index;

			// Create a new point (since someone will wish to look at the point in the original position in future)
			*index = new_index;
			point_domain.push(next_id.next_id(), pos);

			// Add a new segment to be created later
			new_segments.push([new_index, original_index])
		}
	}

	fn update_existing_segments(vector_data: &mut VectorData, vector_data_transform: DAffine2, distance: f64, segments_connected: &mut [usize]) -> Vec<[usize; 2]> {
		let mut next_id = vector_data.point_domain.next_id();
		let mut new_segments = Vec::new();

		for (handles, start_point_index, end_point_index) in vector_data.segment_domain.handles_and_points_mut() {
			// Convert the original segment to a bezier
			let mut bezier = bezier_rs::Bezier {
				start: vector_data.point_domain.positions()[*start_point_index],
				end: vector_data.point_domain.positions()[*end_point_index],
				handles: *handles,
			};

			if bezier.is_linear() {
				bezier.handles = bezier_rs::BezierHandles::Linear;
			}
			bezier = bezier.apply_transformation(|p| vector_data_transform.transform_point2(p));
			let inverse_transform = (vector_data_transform.matrix2.determinant() != 0.).then(|| vector_data_transform.inverse()).unwrap_or_default();

			let original_length = bezier.length(None);
			let mut length = original_length;

			// Only split if the length is big enough to make it worthwhile
			let valid_length = length > 1e-10;
			if segments_connected[*start_point_index] > 0 && valid_length {
				// Apply the bevel to the start
				let distance = distance.min(original_length / 2.);
				bezier = split_distance(bezier, distance, length);
				length = (length - distance).max(0.);
				// Update the start position
				let pos = inverse_transform.transform_point2(bezier.start);
				create_or_modify_point(&mut vector_data.point_domain, segments_connected, pos, start_point_index, &mut next_id, &mut new_segments);
			}

			// Only split if the length is big enough to make it worthwhile
			let valid_length = length > 1e-10;
			if segments_connected[*end_point_index] > 0 && valid_length {
				// Apply the bevel to the end
				let distance = distance.min(original_length / 2.);
				bezier = split_distance(bezier.reversed(), distance, length).reversed();
				// Update the end position
				let pos = inverse_transform.transform_point2(bezier.end);
				create_or_modify_point(&mut vector_data.point_domain, segments_connected, pos, end_point_index, &mut next_id, &mut new_segments);
			}
			// Update the handles
			*handles = bezier.handles.apply_transformation(|p| inverse_transform.transform_point2(p));
		}
		new_segments
	}

	fn insert_new_segments(vector_data: &mut VectorData, new_segments: &[[usize; 2]]) {
		let mut next_id = vector_data.segment_domain.next_id();
		for &[start, end] in new_segments {
			vector_data.segment_domain.push(next_id.next_id(), start, end, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
		}
	}

	let mut segments_connected = segments_connected_count(&vector_data);
	let new_segments = update_existing_segments(&mut vector_data, vector_data_transform, distance, &mut segments_connected);
	insert_new_segments(&mut vector_data, &new_segments);

	vector_data
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
fn bevel(_: impl Ctx, source: VectorDataTable, #[default(10.)] distance: Length) -> VectorDataTable {
	let source_transform = source.transform();
	let source = source.one_instance().instance;

	let mut result = VectorDataTable::new(bevel_algorithm(source.clone(), source_transform, distance));
	*result.transform_mut() = source_transform;
	result
}

#[node_macro::node(name("Merge by Distance"), category("Vector"), path(graphene_core::vector))]
fn merge_by_distance(_: impl Ctx, source: VectorDataTable, #[default(10.)] distance: Length) -> VectorDataTable {
	let source_transform = source.transform();
	let mut source = source.one_instance().instance.clone();

	source.merge_by_distance(distance);

	let mut result = VectorDataTable::new(source);
	*result.transform_mut() = source_transform;

	result
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn area(ctx: impl Ctx + CloneVarArgs + ExtractAll, vector_data: impl Node<Context<'static>, Output = VectorDataTable>) -> f64 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector_data = vector_data.eval(new_ctx).await;

	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	let mut area = 0.;
	let scale = vector_data_transform.decompose_scale();
	for subpath in vector_data.stroke_bezier_paths() {
		area += subpath.area(Some(1e-3), Some(1e-3));
	}

	area * scale[0] * scale[1]
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn centroid(ctx: impl Ctx + CloneVarArgs + ExtractAll, vector_data: impl Node<Context<'static>, Output = VectorDataTable>, centroid_type: CentroidType) -> DVec2 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector_data = vector_data.eval(new_ctx).await;

	let vector_data_transform = vector_data.transform();
	let vector_data = vector_data.one_instance().instance;

	if centroid_type == CentroidType::Area {
		let mut area = 0.;
		let mut centroid = DVec2::ZERO;
		for subpath in vector_data.stroke_bezier_paths() {
			if let Some((subpath_centroid, subpath_area)) = subpath.area_centroid_and_area(Some(1e-3), Some(1e-3)) {
				if subpath_area == 0. {
					continue;
				}
				area += subpath_area;
				centroid += subpath_area * subpath_centroid;
			}
		}

		if area != 0. {
			centroid /= area;
			return vector_data_transform.transform_point2(centroid);
		}
	}

	let mut length = 0.;
	let mut centroid = DVec2::ZERO;
	for subpath in vector_data.stroke_bezier_paths() {
		if let Some((subpath_centroid, subpath_length)) = subpath.length_centroid_and_length(None, true) {
			length += subpath_length;
			centroid += subpath_length * subpath_centroid;
		}
	}

	if length != 0. {
		centroid /= length;
		return vector_data_transform.transform_point2(centroid);
	}

	let positions = vector_data.point_domain.positions();
	if !positions.is_empty() {
		let centroid = positions.iter().sum::<DVec2>() / (positions.len() as f64);
		return vector_data_transform.transform_point2(centroid);
	}

	DVec2::ZERO
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::Node;
	use bezier_rs::Bezier;
	use std::pin::Pin;

	#[derive(Clone)]
	pub struct FutureWrapperNode<T: Clone>(T);

	impl<'i, T: 'i + Clone + Send> Node<'i, Footprint> for FutureWrapperNode<T> {
		type Output = Pin<Box<dyn core::future::Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, _input: Footprint) -> Self::Output {
			let value = self.0.clone();
			Box::pin(async move { value })
		}
	}

	fn vector_node(data: Subpath<PointId>) -> VectorDataTable {
		VectorDataTable::new(VectorData::from_subpath(data))
	}

	#[tokio::test]
	async fn repeat() {
		let direction = DVec2::X * 1.5;
		let instances = 3;
		let repeated = super::repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)), direction, 0., instances).await;
		let vector_data = super::flatten_vector_elements(Footprint::default(), repeated).await;
		let vector_data = vector_data.instances().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 3);
		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}
	#[tokio::test]
	async fn repeat_transform_position() {
		let direction = DVec2::new(12., 10.);
		let instances = 8;
		let repeated = super::repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)), direction, 0., instances).await;
		let vector_data = super::flatten_vector_elements(Footprint::default(), repeated).await;
		let vector_data = vector_data.instances().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 8);
		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}
	#[tokio::test]
	async fn circular_repeat() {
		let repeated = super::circular_repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)), 45., 4., 8).await;
		let vector_data = super::flatten_vector_elements(Footprint::default(), repeated).await;
		let vector_data = vector_data.instances().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 8);

		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (subpath.manipulator_groups()[0].anchor + subpath.manipulator_groups()[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}
	#[tokio::test]
	async fn bounding_box() {
		let bounding_box = super::bounding_box((), vector_node(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE))).await;
		let bounding_box = bounding_box.instances().next().unwrap().instance;
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		assert_eq!(&subpath.anchors()[..4], &[DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.),]);

		// Test a VectorData with non-zero rotation
		let square = VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE));
		let mut square = VectorDataTable::new(square);
		*square.one_instance_mut().transform *= DAffine2::from_angle(core::f64::consts::FRAC_PI_4);
		let bounding_box = BoundingBoxNode {
			vector_data: FutureWrapperNode(square),
		}
		.eval(Footprint::default())
		.await;
		let bounding_box = bounding_box.instances().next().unwrap().instance;
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		let sqrt2 = core::f64::consts::SQRT_2;
		let sqrt2_bounding_box = [DVec2::new(-sqrt2, -sqrt2), DVec2::new(sqrt2, -sqrt2), DVec2::new(sqrt2, sqrt2), DVec2::new(-sqrt2, sqrt2)];
		assert!(subpath.anchors()[..4].iter().zip(sqrt2_bounding_box).all(|(p1, p2)| p1.abs_diff_eq(p2, f64::EPSILON)));
	}
	#[tokio::test]
	async fn copy_to_points() {
		let points = Subpath::new_rect(DVec2::NEG_ONE * 10., DVec2::ONE * 10.);
		let instance = Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE);

		let expected_points = VectorData::from_subpath(points.clone()).point_domain.positions().to_vec();

		let copy_to_points = super::copy_to_points(Footprint::default(), vector_node(points), vector_node(instance), 1., 1., 0., 0, 0., 0).await;
		let flatten_vector_elements = super::flatten_vector_elements(Footprint::default(), copy_to_points).await;
		let flattened_copy_to_points = flatten_vector_elements.instances().next().unwrap().instance;

		assert_eq!(flattened_copy_to_points.region_bezier_paths().count(), expected_points.len());

		for (index, (_, subpath)) in flattened_copy_to_points.region_bezier_paths().enumerate() {
			let offset = expected_points[index];
			assert_eq!(
				&subpath.anchors(),
				&[offset + DVec2::NEG_ONE, offset + DVec2::new(1., -1.), offset + DVec2::ONE, offset + DVec2::new(-1., 1.),]
			);
		}
	}
	#[tokio::test]
	async fn sample_points() {
		let path = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let sample_points = super::sample_points(Footprint::default(), vector_node(path), 30., 0., 0., false, vec![100.]).await;
		let sample_points = sample_points.instances().next().unwrap().instance;
		assert_eq!(sample_points.point_domain.positions().len(), 4);
		for (pos, expected) in sample_points.point_domain.positions().iter().zip([DVec2::X * 0., DVec2::X * 30., DVec2::X * 60., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn adaptive_spacing() {
		let path = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let sample_points = super::sample_points(Footprint::default(), vector_node(path), 18., 45., 10., true, vec![100.]).await;
		let sample_points = sample_points.instances().next().unwrap().instance;
		assert_eq!(sample_points.point_domain.positions().len(), 4);
		for (pos, expected) in sample_points.point_domain.positions().iter().zip([DVec2::X * 45., DVec2::X * 60., DVec2::X * 75., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn poisson() {
		let sample_points = super::poisson_disk_points(
			Footprint::default(),
			vector_node(Subpath::new_ellipse(DVec2::NEG_ONE * 50., DVec2::ONE * 50.)),
			10. * std::f64::consts::SQRT_2,
			0,
		)
		.await;
		let sample_points = sample_points.instances().next().unwrap().instance;
		assert!(
			(20..=40).contains(&sample_points.point_domain.positions().len()),
			"actual len {}",
			sample_points.point_domain.positions().len()
		);
		for point in sample_points.point_domain.positions() {
			assert!(point.length() < 50. + 1., "Expected point in circle {point}")
		}
	}
	#[tokio::test]
	async fn lengths() {
		let subpath = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let lengths = subpath_segment_lengths(Footprint::default(), vector_node(subpath)).await;
		assert_eq!(lengths, vec![100.]);
	}
	#[tokio::test]
	async fn spline() {
		let spline = super::spline(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.))).await;
		let spline = spline.instances().next().unwrap().instance;
		assert_eq!(spline.stroke_bezier_paths().count(), 1);
		assert_eq!(spline.point_domain.positions(), &[DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)]);
	}
	#[tokio::test]
	async fn morph() {
		let source = Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.);
		let target = Subpath::new_ellipse(DVec2::NEG_ONE * 100., DVec2::ZERO);
		let sample_points = super::morph(Footprint::default(), vector_node(source), vector_node(target), 0.5, 0).await;
		let sample_points = sample_points.instances().next().unwrap().instance;
		assert_eq!(
			&sample_points.point_domain.positions()[..4],
			vec![DVec2::new(-25., -50.), DVec2::new(50., -25.), DVec2::new(25., 50.), DVec2::new(-50., 25.)]
		);
	}

	#[track_caller]
	fn contains_segment(vector: VectorData, target: bezier_rs::Bezier) {
		let segments = vector.segment_bezier_iter().map(|x| x.1);
		let count = segments.filter(|bezier| bezier.abs_diff_eq(&target, 0.01) || bezier.reversed().abs_diff_eq(&target, 0.01)).count();
		assert_eq!(
			count,
			1,
			"Expected exactly one matching segment for {target:?}, but found {count}. The given segments are: {:#?}",
			vector.segment_bezier_iter().collect::<Vec<_>>()
		);
	}

	#[tokio::test]
	async fn bevel_rect() {
		let source = Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 5.);
		let beveled = beveled.instances().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 8);
		assert_eq!(beveled.segment_domain.ids().len(), 8);

		// Segments
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(5., 0.), DVec2::new(95., 0.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(5., 100.), DVec2::new(95., 100.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(0., 5.), DVec2::new(0., 95.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(100., 5.), DVec2::new(100., 95.)));

		// Joins
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(5., 0.), DVec2::new(0., 5.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(95., 0.), DVec2::new(100., 5.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(100., 95.), DVec2::new(95., 100.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(5., 100.), DVec2::new(0., 95.)));
	}

	#[tokio::test]
	async fn bevel_open_curve() {
		let curve = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 100.), DVec2::X * 100.);
		let source = Subpath::from_beziers(&[Bezier::from_linear_dvec2(DVec2::X * -100., DVec2::ZERO), curve], false);
		let beveled = super::bevel((), vector_node(source), 5.);
		let beveled = beveled.instances().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(-100., 0.)));
		let trimmed = curve.trim(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))), bezier_rs::TValue::Parametric(1.));
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-5., 0.), trimmed.start));
	}

	#[tokio::test]
	async fn bevel_with_transform() {
		let curve = Bezier::from_cubic_dvec2(DVec2::new(0., 0.), DVec2::new(1., 0.), DVec2::new(1., 10.), DVec2::new(10., 0.));
		let source = Subpath::<PointId>::from_beziers(&[Bezier::from_linear_dvec2(DVec2::new(-10., 0.), DVec2::ZERO), curve], false);
		let vector_data = VectorData::from_subpath(source);
		let mut vector_data_table = VectorDataTable::new(vector_data.clone());

		*vector_data_table.one_instance_mut().transform = DAffine2::from_scale_angle_translation(DVec2::splat(10.), 1., DVec2::new(99., 77.));

		let beveled = super::bevel((), VectorDataTable::new(vector_data), 5.);
		let beveled = beveled.instances().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(-10., 0.)));
		let trimmed = curve.trim(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))), bezier_rs::TValue::Parametric(1.));
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-5., 0.), trimmed.start));
	}

	#[tokio::test]
	async fn bevel_too_high() {
		let source = Subpath::from_anchors([DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)], false);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 999.);
		let beveled = beveled.instances().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);

		// Segments
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(0., 0.), DVec2::new(50., 0.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(100., 50.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(50., 100.)));

		// Joins
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(50., 0.), DVec2::new(100., 50.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(50., 100.)));
	}

	#[tokio::test]
	async fn bevel_repeated_point() {
		let curve = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 100.), DVec2::X * 100.);
		let point = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::ZERO, DVec2::ZERO);
		let source = Subpath::from_beziers(&[Bezier::from_linear_dvec2(DVec2::X * -100., DVec2::ZERO), point, curve], false);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 5.);
		let beveled = beveled.instances().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);

		// Segments
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-100., 0.), DVec2::new(-5., 0.)));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(0., 0.)));
		contains_segment(beveled.clone(), point);
		let [start, end] = curve.split(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))));
		contains_segment(beveled.clone(), bezier_rs::Bezier::from_linear_dvec2(start.start, start.end));
		contains_segment(beveled.clone(), end);
	}
}
