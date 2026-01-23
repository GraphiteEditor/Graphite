use core::f64;
use core_types::color::Color;
use core_types::registry::types::{Angle, Multiplier, SeedValue};
use core_types::table::Table;
use core_types::transform::{ApplyTransform, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, InjectVarArgs, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
use rand::{Rng, SeedableRng};
use std::f64::consts::TAU;
use vector_types::GradientStops;

/// Applies the specified transform to the input value, which may be a graphic type or another transform.
#[node_macro::node(category(""))]
async fn transform<T: ApplyTransform + 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	#[implementations(
		Context -> DAffine2,
		Context -> DVec2,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	content: impl Node<Context<'static>, Output = T>,
	translation: DVec2,
	rotation: f64,
	scale: DVec2,
	skew: DVec2,
) -> T {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut transform_target = content.eval(ctx.into_context()).await;

	transform_target.left_apply_transform(&matrix);

	transform_target
}

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Math: Transform"))]
fn reset_transform<T>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut content: Table<T>,
	#[default(true)] reset_translation: bool,
	reset_rotation: bool,
	reset_scale: bool,
) -> Table<T> {
	for row in content.iter_mut() {
		// Translation
		if reset_translation {
			row.transform.translation = DVec2::ZERO;
		}
		// (Rotation, Scale)
		match (reset_rotation, reset_scale) {
			(true, true) => {
				row.transform.matrix2 = DMat2::IDENTITY;
			}
			(true, false) => {
				let scale = row.transform.decompose_scale();
				row.transform.matrix2 = DMat2::from_diagonal(scale);
			}
			(false, true) => {
				let rotation = row.transform.decompose_rotation();
				let rotation_matrix = DMat2::from_angle(rotation);
				row.transform.matrix2 = rotation_matrix;
			}
			(false, false) => {}
		}
	}
	content
}

/// Overwrites the transform of each element in the input table with the specified transform.
#[node_macro::node(category("Math: Transform"))]
fn replace_transform<T>(
	_: impl Ctx + InjectFootprint,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut content: Table<T>,
	transform: DAffine2,
) -> Table<T> {
	for row in content.iter_mut() {
		*row.transform = transform.transform();
	}
	content
}

// TODO: Figure out how this node should behave once #2982 is implemented.
/// Obtains the transform of the first element in the input table, if present.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
async fn extract_transform<T>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	content: Table<T>,
) -> DAffine2 {
	content.iter().next().map(|row| *row.transform).unwrap_or_default()
}

/// Produces the inverse of the input transform, which is the transform that undoes the effect of the original transform.
#[node_macro::node(category("Math: Transform"))]
fn invert_transform(_: impl Ctx, transform: DAffine2) -> DAffine2 {
	transform.inverse()
}

/// Extracts the translation component from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_translation(_: impl Ctx, transform: DAffine2) -> DVec2 {
	transform.translation
}

/// Extracts the rotation component (in degrees) from the input transform.
/// This, together with the "Decompose Scale" node, also may jointly represent any shear component in the original transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_rotation().to_degrees()
}

/// Extracts the scale component from the input transform.
/// This, together with the "Decompose Rotation" node, also may jointly represent any shear component in the original transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: DAffine2) -> DVec2 {
	transform.decompose_scale()
}

// TODO: Split into "Assign Translations", "Assign Rotations", and "Assign Scales" nodes.
#[node_macro::node(name("Copy to Points"), category("Instancing"), path(core_types::vector))]
async fn copy_to_points<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Sync + Ctx + InjectVarArgs,
	points: Table<Vector>,
	/// Artwork to be copied and placed at each point.
	#[expose]
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	instance: impl Node<'n, Context<'static>, Output = Table<T>>,
	// TODO: Split the next 4 parameters into an "Assign Scale" node which (like "Assign Colors") will also have a "Randomize" parameter as well as a "Repeat Every" parameter. If not randomized, it assigns from min to max scale.
	// TODO: Fix the node macro to support #[default(1)] on `impl Node` parameters
	/// Minimum range of randomized sizes given to each instance.
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_min: impl Node<'n, Context<'static>, Output = Multiplier>,
	/// Maximum range of randomized sizes given to each instance.
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_max: impl Node<'n, Context<'static>, Output = Multiplier>,
	/// Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes).
	#[range((-50., 50.))]
	random_scale_bias: impl Node<'n, Context<'static>, Output = f64>,
	/// Seed to determine unique variations on all the randomized instance sizes.
	random_scale_seed: impl Node<'n, Context<'static>, Output = SeedValue>,
	// TODO: Split the next 2 parameters into an "Assign Rotation" node which (like "Assign Colors") will also have a "Randomize" parameter as well as a "Repeat Every" parameter. If not randomized, it assigns from min to max rotation.
	// TODO: Also add a node called "Assign Translation" which is basically the Repeat/Array node.
	// TODO: Also each of these "Assign" nodes can be renamed to "Spread".
	/// Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise.
	#[range((0., 360.))]
	random_rotation: impl Node<'n, Context<'static>, Output = Angle>,
	/// Seed to determine unique variations on all the randomized instance angles.
	random_rotation_seed: impl Node<'n, Context<'static>, Output = SeedValue>,
) -> Table<T> {
	let mut result_table = Table::new();

	let mut index: u64 = 0;

	for row in points.into_iter() {
		let points_transform = row.transform;
		for &point in row.element.point_domain.positions() {
			let translation = points_transform.transform_point2(point);

			let context = OwnedContextImpl::from(ctx.clone()).with_index(index as usize).with_position(translation).into_context();

			let random_rotation = random_rotation.eval(context.clone()).await;
			let rotation = if random_rotation.abs() > 1e-6 {
				let random_rotation_seed = random_rotation_seed.eval(context.clone()).await;
				let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(random_rotation_seed as u64 + index);
				let degrees = (rotation_rng.random::<f64>() - 0.5) * random_rotation;
				degrees / 360. * TAU
			} else {
				0.
			};

			let random_scale_max = random_scale_max.eval(context.clone()).await;
			let random_scale_min = random_scale_min.eval(context.clone()).await;
			let random_scale_difference = random_scale_max - random_scale_min;

			let scale = if random_scale_difference.abs() > 1e-6 {
				let random_scale_seed = random_scale_seed.eval(context.clone()).await;
				let mut scale_rng = rand::rngs::StdRng::seed_from_u64(random_scale_seed as u64 + index);

				let random_scale_bias = random_scale_bias.eval(context.clone()).await;
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

			let generated_instance = instance.eval(context).await;
			index += 1;

			for mut row in generated_instance.iter().map(|row| row.into_cloned()) {
				row.transform.translation = DVec2::ZERO;
				row.transform = transform * row.transform;

				result_table.push(row);
			}
		}
	}

	result_table
}
