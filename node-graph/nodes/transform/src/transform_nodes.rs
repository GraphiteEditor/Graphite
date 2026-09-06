use core::f64;
use core_types::attribute::{Attr, Transform as TransformAttr};
use core_types::color::Color;
use core_types::extent::{ExtentIn, LevelIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, Interrupt};
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{CacheHash, Context, Ctx, DeriveCtx, InjectFootprint, ModifyFootprint};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Applies the specified transform to each lane of the input, composing onto the lane's transform attribute.
#[node_macro::node(category("Math: Transform"), extent(transform_extent))]
fn transform<T>(
	ctx: impl Ctx + DeriveCtx + ModifyFootprint,
	content: impl Node<Context<'_>, Output = (T, Attr<TransformAttr>)>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: f64,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: DVec2,
) -> Result<(T, Attr<TransformAttr>), Interrupt> {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let transformed = ctx.modify_footprint(|footprint| footprint.apply_transform(&matrix));
	let (element, transform) = content.eval(&transformed.ctx())?;

	Ok((element, Attr(matrix * *transform)))
}

fn transform_extent(content: ExtentIn<'_>, _translation: ValueIn<'_, DVec2>, _rotation: ValueIn<'_, f64>, _scale: ValueIn<'_, DVec2>, _skew: ValueIn<'_, DVec2>, level: LevelIn) -> GPoll<Extent> {
	content.at(level)
}

/// The transform applied to a plain transform or point value. Registered under the same identifier
/// as the leveled `transform`, serving its value-typed rows.
#[node_macro::node(category(""))]
fn transform_value<T: ApplyTransform + 'static>(
	ctx: impl Ctx + DeriveCtx + ModifyFootprint,
	#[implementations(Context -> DAffine2, Context -> DVec2)] content: impl Node<Context<'_>, Output = T>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: f64,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: DVec2,
) -> Result<T, Interrupt> {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let transformed = ctx.modify_footprint(|footprint| footprint.apply_transform(&matrix));
	let mut transform_target = content.eval(&transformed.ctx())?;

	transform_target.left_apply_transform(&matrix);

	Ok(transform_target)
}

pub use _transform_value_mod::transform_value_entries;

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Math: Transform"))]
fn reset_transform<T>(_: impl Ctx, (element, transform): (T, Attr<TransformAttr>), #[default(true)] reset_translation: bool, reset_rotation: bool, reset_scale: bool) -> (T, Attr<TransformAttr>) {
	let mut row_transform = *transform;
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
	(element, Attr(row_transform))
}

/// Overwrites the transform of each lane of the input with the specified transform.
#[node_macro::node(category("Math: Transform"))]
fn replace_transform<T>(_: impl Ctx + InjectFootprint, (element, _content_transform): (T, Attr<TransformAttr>), transform: DAffine2) -> (T, Attr<TransformAttr>) {
	(element, Attr(transform))
}

// TODO: Figure out how this node should behave once #2982 is implemented.
/// Obtains the transform of the first lane of the input, if present.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
fn extract_transform<T: Clone + Send + Sync + CacheHash + 'static>(_: impl Ctx, #[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, GradientStops)] content: IList<T>) -> DAffine2 {
	match content.len() {
		0 => DAffine2::default(),
		_ => content.lane(0).attr::<TransformAttr>(),
	}
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
