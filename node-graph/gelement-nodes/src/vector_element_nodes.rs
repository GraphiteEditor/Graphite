use glam::{DAffine2, DVec2};
use graphene_core::color::Color;
use graphene_core::context::Ctx;
use graphene_core::gradient::{Gradient, GradientStops};
use graphene_core::instances::{InstanceMut, Instances};
use graphene_core::registry::types::{Angle, IntegerCount, Multiplier, PixelSize, SeedValue};
use graphene_element::{GraphicElement, GraphicGroupTable};
use graphene_raster::{CPU, GPU, RasterDataTable};
use graphene_vector::reference_point::ReferencePoint;
use graphene_vector::style::{Fill, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_vector::{VectorData, VectorDataTable};
use rand::{Rng, SeedableRng};
use std::f64::consts::TAU;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Implemented for types that can be converted to an iterator of vector data.
/// Used for the fill and stroke node so they can be used on VectorData or GraphicGroup
trait VectorDataTableIterMut {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<'_, VectorData>>;
}

impl VectorDataTableIterMut for GraphicGroupTable {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<'_, VectorData>> {
		// Grab only the direct children
		self.instance_mut_iter()
			.filter_map(|element| element.instance.as_vector_data_mut())
			.flat_map(move |vector_data| vector_data.instance_mut_iter())
	}
}

impl VectorDataTableIterMut for VectorDataTable {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = InstanceMut<'_, VectorData>> {
		self.instance_mut_iter()
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

		let color = gradient.evaluate(factor);

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
	/// The alignment of stroke to the path's centerline or (for closed shapes) the inside or outside of the shape.
	align: StrokeAlign,
	/// The shape of the stroke at open endpoints.
	cap: StrokeCap,
	/// The curvature of the bent stroke at sharp corners.
	join: StrokeJoin,
	#[default(4.)]
	/// The threshold for when a miter-joined stroke is converted to a bevel-joined stroke when a sharp angle becomes pointier than this ratio.
	miter_limit: f64,
	/// The order to paint the stroke on top of the fill, or the fill on top of the stroke.
	/// <https://svgwg.org/svg2-draft/painting.html#PaintOrderProperty>
	paint_order: PaintOrder,
	/// The stroke dash lengths. Each length forms a distance in a pattern where the first length is a dash, the second is a gap, and so on. If the list is an odd length, the pattern repeats with solid-gap roles reversed.
	dash_lengths: Vec<f64>,
	/// The phase offset distance from the starting point of the dash pattern.
	dash_offset: f64,
) -> Instances<V>
where
	Instances<V>: VectorDataTableIterMut + 'n + Send,
{
	let stroke = Stroke {
		color: color.into(),
		weight,
		dash_lengths,
		dash_offset,
		cap,
		join,
		join_miter_limit: miter_limit,
		align,
		transform: DAffine2::IDENTITY,
		non_scaling: false,
		paint_order,
	};

	for vector in vector_data.vector_iter_mut() {
		let mut stroke = stroke.clone();
		stroke.transform *= *vector.transform;
		vector.instance.style.set_stroke(stroke);
	}

	vector_data
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)] instance: Instances<I>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(4)] instances: IntegerCount,
) -> Instances<I> {
	let angle = angle.to_radians();
	let count = instances.max(1);
	let total = (count - 1) as f64;

	let mut result_table = Instances::<I>::default();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		for instance in instance.instance_ref_iter() {
			let mut instance = instance.to_instance_cloned();

			let local_translation = DAffine2::from_translation(instance.transform.translation);
			let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
			instance.transform = local_translation * transform * local_matrix;

			result_table.push(instance);
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn circular_repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)] instance: Instances<I>,
	angle_offset: Angle,
	#[default(5)] radius: f64,
	#[default(5)] instances: IntegerCount,
) -> Instances<I> {
	let count = instances.max(1);

	let mut result_table = Instances::<I>::default();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + angle_offset.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		for instance in instance.instance_ref_iter() {
			let mut instance = instance.to_instance_cloned();

			let local_translation = DAffine2::from_translation(instance.transform.translation);
			let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
			instance.transform = local_translation * transform * local_matrix;

			result_table.push(instance);
		}
	}

	result_table
}

#[node_macro::node(name("Copy to Points"), category("Instancing"), path(graphene_core::vector))]
async fn copy_to_points<I: 'n + Send + Clone>(
	_: impl Ctx,
	points: VectorDataTable,
	#[expose]
	/// Artwork to be copied and placed at each point.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)]
	instance: Instances<I>,
	/// Minimum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_min: Multiplier,
	/// Maximum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_max: Multiplier,
	/// Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes).
	#[range((-50., 50.))]
	random_scale_bias: f64,
	/// Seed to determine unique variations on all the randomized instance sizes.
	random_scale_seed: SeedValue,
	/// Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise.
	#[range((0., 360.))]
	random_rotation: Angle,
	/// Seed to determine unique variations on all the randomized instance angles.
	random_rotation_seed: SeedValue,
) -> Instances<I> {
	let mut result_table = Instances::<I>::default();

	let random_scale_difference = random_scale_max - random_scale_min;

	for point_instance in points.instance_iter() {
		let mut scale_rng = rand::rngs::StdRng::seed_from_u64(random_scale_seed.into());
		let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(random_rotation_seed.into());

		let do_scale = random_scale_difference.abs() > 1e-6;
		let do_rotation = random_rotation.abs() > 1e-6;

		let points_transform = point_instance.transform;
		for &point in point_instance.instance.point_domain.positions() {
			let translation = points_transform.transform_point2(point);

			let rotation = if do_rotation {
				let degrees = (rotation_rng.random::<f64>() - 0.5) * random_rotation;
				degrees / 360. * TAU
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

			let transform = DAffine2::from_scale_angle_translation(DVec2::splat(scale), rotation, translation);

			for mut instance in instance.instance_ref_iter().map(|instance| instance.to_instance_cloned()) {
				let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
				instance.transform = transform * local_matrix;

				result_table.push(instance);
			}
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn mirror<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)] instance: Instances<I>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	offset: f64,
	#[range((-90., 90.))] angle: Angle,
	#[default(true)] keep_original: bool,
) -> Instances<I> {
	let mut result_table = Instances::default();

	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference is based on the bounding box (at least for now, until we have proper local layer origins)
	let Some(bounding_box) = instance.bounding_box(DAffine2::IDENTITY, false) else {
		return result_table;
	};

	let reference_point_location = relative_to_bounds.point_in_bounding_box((bounding_box[0], bounding_box[1]).into());
	let mirror_reference_point = reference_point_location.map(|point| point + normal * offset);

	// Create the reflection matrix
	let reflection = DAffine2::from_mat2_translation(
		glam::DMat2::from_cols(
			DVec2::new(1. - 2. * normal.x * normal.x, -2. * normal.y * normal.x),
			DVec2::new(-2. * normal.x * normal.y, 1. - 2. * normal.y * normal.y),
		),
		DVec2::ZERO,
	);

	// Apply reflection around the reference point
	let reflected_transform = if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	};

	// Add original instance depending on the keep_original flag
	if keep_original {
		for instance in instance.clone().instance_iter() {
			result_table.push(instance);
		}
	}

	// Create and add mirrored instance
	for mut instance in instance.instance_iter() {
		instance.transform = reflected_transform * instance.transform;
		instance.source_node_id = None;
		result_table.push(instance);
	}

	result_table
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn flatten_path<I: 'n + Send>(_: impl Ctx, #[implementations(GraphicGroupTable, VectorDataTable)] graphic_group_input: Instances<I>) -> VectorDataTable {
	// A node based solution to support passing through vector data could be a network node with a cache node connected to
	// a Flatten Path connected to an if else node, another connection from the cache directly
	// To the if else node, and another connection from the cache to a matches type node connected to the if else node.
	fn flatten_group(graphic_group_table: &GraphicGroupTable, output: &mut InstanceMut<VectorData>) {
		for (group_index, current_element) in graphic_group_table.instance_ref_iter().enumerate() {
			match current_element.instance {
				GraphicElement::VectorData(vector_data_table) => {
					// Loop through every row of the VectorDataTable and concatenate each instance's subpath into the output VectorData instance.
					for (vector_index, vector_data_instance) in vector_data_table.instance_ref_iter().enumerate() {
						let other = vector_data_instance.instance;
						let transform = *current_element.transform * *vector_data_instance.transform;
						let node_id = current_element.source_node_id.map(|node_id| node_id.0).unwrap_or_default();

						let mut hasher = DefaultHasher::new();
						(group_index, vector_index, node_id).hash(&mut hasher);
						let collision_hash_seed = hasher.finish();

						output.instance.concat(other, transform, collision_hash_seed);

						// Use the last encountered style as the output style
						output.instance.style = vector_data_instance.instance.style.clone();
					}
				}
				GraphicElement::GraphicGroup(graphic_group) => {
					let mut graphic_group = graphic_group.clone();
					for instance in graphic_group.instance_mut_iter() {
						*instance.transform = *current_element.transform * *instance.transform;
					}

					flatten_group(&graphic_group, output);
				}
				_ => {}
			}
		}
	}

	// Create a table with one instance of an empty VectorData, then get a mutable reference to it which we append flattened subpaths to
	let mut output_table = VectorDataTable::new(VectorData::default());
	let Some(mut output) = output_table.instance_mut_iter().next() else {
		return output_table;
	};

	// Flatten the graphic group input into the output VectorData instance
	let base_graphic_group = GraphicGroupTable::new(graphic_group_input.to_graphic_element());
	flatten_group(&base_graphic_group, &mut output);

	// Return the single-row VectorDataTable containing the flattened VectorData subpaths
	output_table
}

#[node_macro::node(category("General"), path(graphene_core::vector))]
async fn count_elements<I>(_: impl Ctx, #[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>, RasterDataTable<GPU>)] source: Instances<I>) -> u64 {
	source.instance_iter().count() as u64
}

#[cfg(test)]
mod tests {
	use super::*;
	use bezier_rs::Subpath;
	use graphene_core::transform::Footprint;
	use graphene_vector::PointId;

	fn vector_node(data: Subpath<PointId>) -> VectorDataTable {
		VectorDataTable::new(VectorData::from_subpath(data))
	}

	#[tokio::test]
	async fn repeat() {
		let direction = DVec2::X * 1.5;
		let instances = 3;
		let repeated = super::repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)), direction, 0., instances).await;
		let vector_data = super::flatten_path(Footprint::default(), repeated).await;
		let vector_data = vector_data.instance_ref_iter().next().unwrap().instance;
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
		let vector_data = super::flatten_path(Footprint::default(), repeated).await;
		let vector_data = vector_data.instance_ref_iter().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 8);
		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}

	#[tokio::test]
	async fn circular_repeat() {
		let repeated = super::circular_repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)), 45., 4., 8).await;
		let vector_data = super::flatten_path(Footprint::default(), repeated).await;
		let vector_data = vector_data.instance_ref_iter().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 8);

		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (subpath.manipulator_groups()[0].anchor + subpath.manipulator_groups()[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}

	#[tokio::test]
	async fn copy_to_points() {
		let points = Subpath::new_rect(DVec2::NEG_ONE * 10., DVec2::ONE * 10.);
		let instance = Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE);

		let expected_points = VectorData::from_subpath(points.clone()).point_domain.positions().to_vec();

		let copy_to_points = super::copy_to_points(Footprint::default(), vector_node(points), vector_node(instance), 1., 1., 0., 0, 0., 0).await;
		let flatten_path = super::flatten_path(Footprint::default(), copy_to_points).await;
		let flattened_copy_to_points = flatten_path.instance_ref_iter().next().unwrap().instance;

		assert_eq!(flattened_copy_to_points.region_bezier_paths().count(), expected_points.len());

		for (index, (_, subpath)) in flattened_copy_to_points.region_bezier_paths().enumerate() {
			let offset = expected_points[index];
			assert_eq!(
				&subpath.anchors(),
				&[offset + DVec2::NEG_ONE, offset + DVec2::new(1., -1.), offset + DVec2::ONE, offset + DVec2::new(-1., 1.),]
			);
		}
	}
}
