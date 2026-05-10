use core::f64;
use core_types::color::Color;
use core_types::list::{Item, List, ListDyn};
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Applies the specified transform to the input value, which may be a graphic type or another transform.
#[node_macro::node(category("Math: Transform"))]
async fn transform<T: ApplyTransform + 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	#[implementations(
		Context -> Item<DAffine2>,
		Context -> Item<DVec2>,
		Context -> Item<List<Graphic>>,
		Context -> Item<List<Vector>>,
		Context -> Item<List<Raster<CPU>>>,
		Context -> Item<List<Raster<GPU>>>,
		Context -> Item<List<Color>>,
		Context -> Item<List<GradientStops>>,
	)]
	content: impl Node<Context<'static>, Output = Item<T>>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: Item<DVec2>,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: Item<DVec2>,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: Item<DVec2>,
) -> Item<T> {
	let translation = translation.into_element();
	let rotation = rotation.into_element();
	let scale = scale.into_element();
	let skew = skew.into_element();

	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut transform_target = content.eval(ctx.into_context()).await.into_element();

	transform_target.left_apply_transform(&matrix);

	Item::new_from_element(transform_target)
}

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Math: Transform"))]
fn reset_transform<T>(
	_: impl Ctx,
	#[implementations(
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<GradientStops>,
	)]
	mut content: List<T>,
	#[default(true)] reset_translation: Item<bool>,
	reset_rotation: Item<bool>,
	reset_scale: Item<bool>,
) -> Item<List<T>> {
	let reset_translation = reset_translation.into_element();
	let reset_rotation = reset_rotation.into_element();
	let reset_scale = reset_scale.into_element();

	for row_transform in content.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
		if reset_translation {
			row_transform.translation = DVec2::ZERO;
		}

		match (reset_rotation, reset_scale) {
			(true, true) => row_transform.matrix2 = DMat2::IDENTITY,
			(true, false) => {
				let scale = row_transform.scale_magnitudes();
				row_transform.matrix2 = DMat2::from_diagonal(scale);
			}
			(false, true) => {
				let rotation = row_transform.decompose_rotation();
				row_transform.matrix2 = DMat2::from_angle(rotation);
			}
			(false, false) => {}
		}
	}
	Item::new_from_element(content)
}

/// Overwrites the transform of each item in the input `List` with the specified transform.
#[node_macro::node(category("Math: Transform"))]
fn replace_transform<T>(
	_: impl Ctx + InjectFootprint,
	#[implementations(
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<GradientStops>,
	)]
	mut content: List<T>,
	transform: Item<DAffine2>,
) -> Item<List<T>> {
	let transform = transform.into_element();

	for row_transform in content.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
		*row_transform = transform.transform();
	}
	Item::new_from_element(content)
}

// TODO: Figure out how this node should behave once #2982 is implemented.
/// Obtains the transform of the first item in the input `List`, if present.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
async fn extract_transform(_: impl Ctx, content: ListDyn) -> Item<DAffine2> {
	Item::new_from_element(content.attribute::<DAffine2>(ATTR_TRANSFORM, 0).copied().unwrap_or_default())
}

/// Produces the inverse of the input transform, which is the transform that undoes the effect of the original transform.
#[node_macro::node(category("Math: Transform"))]
fn invert_transform(_: impl Ctx, transform: DAffine2) -> Item<DAffine2> {
	Item::new_from_element(transform.inverse())
}

/// Extracts the translation component from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_translation(_: impl Ctx, transform: DAffine2) -> Item<DVec2> {
	Item::new_from_element(transform.translation)
}

/// Extracts the rotation component (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: DAffine2) -> Item<f64> {
	Item::new_from_element(transform.decompose_rotation().to_degrees())
}

/// Extracts the scale component from the input transform.
/// **Magnitude** returns the visual length of each axis (always positive, includes any skew contribution).
/// **Pure** returns the isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: DAffine2, scale_type: Item<ScaleType>) -> Item<DVec2> {
	let scale_type = scale_type.into_element();

	Item::new_from_element(match scale_type {
		ScaleType::Magnitude => transform.scale_magnitudes(),
		ScaleType::Pure => transform.decompose_scale(),
	})
}

/// Extracts the skew angle (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_skew(_: impl Ctx, transform: DAffine2) -> Item<f64> {
	Item::new_from_element(transform.decompose_skew().atan().to_degrees())
}
