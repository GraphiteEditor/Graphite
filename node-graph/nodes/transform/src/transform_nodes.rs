use core::f64;
use core_types::color::Color;
use core_types::list::{Item, ListDyn};
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Applies the specified transform to the input content.
#[node_macro::node(category("Math: Transform"))]
async fn transform<T: 'n + Send + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	#[implementations(
		Context -> Item<DAffine2>,
		Context -> Item<DVec2>,
		Context -> Item<Graphic>,
		Context -> Item<String>,
		Context -> Item<Vector>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<GradientStops>,
	)]
	content: impl Node<Context<'static>, Output = Item<T>>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: Item<DVec2>,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: Item<DVec2>,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: Item<DVec2>,
) -> Item<T> {
	let (translation, rotation, scale, skew) = (*translation.element(), *rotation.element(), *scale.element(), *skew.element());

	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut item = content.eval(ctx.into_context()).await;

	item.left_apply_transform(&matrix);

	item
}

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Math: Transform"))]
fn reset_transform<T>(
	_: impl Ctx,
	#[implementations(
		Graphic,
		Vector,
		Raster<CPU>,
		Raster<GPU>,
		Color,
		GradientStops,
	)]
	mut content: Item<T>,
	#[default(true)] reset_translation: bool,
	reset_rotation: bool,
	reset_scale: bool,
) -> Item<T> {
	let item_transform = content.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM);

	if reset_translation {
		item_transform.translation = DVec2::ZERO;
	}

	match (reset_rotation, reset_scale) {
		(true, true) => item_transform.matrix2 = DMat2::IDENTITY,
		(true, false) => {
			let scale = item_transform.scale_magnitudes();
			item_transform.matrix2 = DMat2::from_diagonal(scale);
		}
		(false, true) => {
			let rotation = item_transform.decompose_rotation();
			item_transform.matrix2 = DMat2::from_angle(rotation);
		}
		(false, false) => {}
	}

	content
}

/// Overwrites the transform of the input content with the specified transform.
#[node_macro::node(category("Math: Transform"))]
fn replace_transform<T>(
	_: impl Ctx + InjectFootprint,
	#[implementations(
		Graphic,
		Vector,
		Raster<CPU>,
		Raster<GPU>,
		Color,
		GradientStops,
	)]
	mut content: Item<T>,
	transform: DAffine2,
) -> Item<T> {
	content.set_attribute(ATTR_TRANSFORM, transform.transform());
	content
}

// TODO: Figure out how this node should behave once #2982 is implemented.
/// Obtains the transform of the first item in the input `List`, if present.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
async fn extract_transform(_: impl Ctx, content: ListDyn) -> DAffine2 {
	content.attribute::<DAffine2>(ATTR_TRANSFORM, 0).copied().unwrap_or_default()
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
#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_rotation().to_degrees()
}

/// Extracts the scale component from the input transform.
/// **Magnitude** returns the visual length of each axis (always positive, includes any skew contribution).
/// **Pure** returns the isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: DAffine2, scale_type: ScaleType) -> DVec2 {
	match scale_type {
		ScaleType::Magnitude => transform.scale_magnitudes(),
		ScaleType::Pure => transform.decompose_scale(),
	}
}

/// Extracts the skew angle (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_skew(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_skew().atan().to_degrees()
}
