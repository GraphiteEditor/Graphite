use core::f64;
use core_types::color::Color;
use core_types::table::Table;
use core_types::transform::{ApplyTransform, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
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
