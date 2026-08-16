use core::f64;
use core_types::color::Color;
use core_types::list::{Item, List};
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use vector_types::Gradient;

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
		Context -> Item<Gradient>,
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

/// The whole-`List` counterpart of `transform`, composing the matrix onto every item of a rank-1 content wire.
/// Registered under the `TransformNode` identifier by manual registry rows, since the macro's element-wise variants require an `Item`-peeling primary.
#[node_macro::node(category(""), skip_impl)]
async fn transform_list<T: 'n + Send + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	content: impl Node<Context<'static>, Output = List<T>>,
	translation: Item<DVec2>,
	rotation: Item<f64>,
	scale: Item<DVec2>,
	skew: Item<DVec2>,
) -> List<T> {
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

	let mut list = content.eval(ctx.into_context()).await;

	list.left_apply_transform(&matrix);

	list
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
		Gradient,
		String,
	)]
	content: Item<T>,
	#[default(true)] reset_translation: Item<bool>,
	reset_rotation: Item<bool>,
	reset_scale: Item<bool>,
) -> Item<T> {
	let mut content = content;
	let (reset_translation, reset_rotation, reset_scale) = (*reset_translation.element(), *reset_rotation.element(), *reset_scale.element());

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
		Gradient,
		String,
	)]
	content: Item<T>,
	transform: Item<DAffine2>,
) -> Item<T> {
	let mut content = content;
	let transform = *transform.element();

	content.set_attribute(ATTR_TRANSFORM, transform.transform());
	content
}

/// Obtains the transform of the input content.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
fn extract_transform<T: 'n + Send>(_: impl Ctx, #[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Artboard)] content: Item<T>) -> Item<DAffine2> {
	Item::new_from_element(content.attribute_cloned_or_default(ATTR_TRANSFORM))
}

/// Produces the inverse of the input transform, which is the transform that undoes the effect of the original transform.
#[node_macro::node(category("Math: Transform"))]
fn invert_transform(_: impl Ctx, transform: Item<DAffine2>) -> Item<DAffine2> {
	let (transform, attributes) = transform.into_parts();

	let result = transform.inverse();

	Item::from_parts(result, attributes)
}

/// Extracts the translation component from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_translation(_: impl Ctx, transform: Item<DAffine2>) -> Item<DVec2> {
	Item::new_from_element(transform.into_element().translation)
}

/// Extracts the rotation component (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: Item<DAffine2>) -> Item<f64> {
	Item::new_from_element(transform.into_element().decompose_rotation().to_degrees())
}

/// Extracts the scale component from the input transform.
/// **Magnitude** returns the visual length of each axis (always positive, includes any skew contribution).
/// **Pure** returns the isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: Item<DAffine2>, scale_type: Item<ScaleType>) -> Item<DVec2> {
	let transform = transform.into_element();
	let scale_type = scale_type.into_element();

	let result = match scale_type {
		ScaleType::Magnitude => transform.scale_magnitudes(),
		ScaleType::Pure => transform.decompose_scale(),
	};

	Item::new_from_element(result)
}

/// Extracts the skew angle (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_skew(_: impl Ctx, transform: Item<DAffine2>) -> Item<f64> {
	Item::new_from_element(transform.into_element().decompose_skew().atan().to_degrees())
}
