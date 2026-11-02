use core::f64;
use core_types::color::Color;
use core_types::list::{Item, List};
use core_types::math::midpoint::apply_midpoint_inverse;
use core_types::math::spread::spread_factors;
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Context, Ctx, ExtractAll, InjectFootprint, Interrupt, ModifyFootprint, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use vector_types::Gradient;

/// Applies the specified transform to the input content.
#[node_macro::node(category("Transform"), icon("NodeTransform"))]
fn transform<T: Send + 'static>(
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
	// TODO: Replace this pair of shear angles with a single `f64` skew angle
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")]
	#[hard(-89.9..89.9)]
	skew: Item<DVec2>,
) -> Result<Item<T>, Interrupt> {
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

	let mut item = content.eval(ctx.into_context())?;

	item.left_apply_transform(&matrix);

	Ok(item)
}

/// The whole-`List` counterpart of `transform`, composing the matrix onto every item of a rank-1 content wire.
/// Registered under the `TransformNode` identifier by manual registry rows, since the macro's element-wise variants require an `Item`-peeling primary.
#[node_macro::node(category(""), skip_impl)]
fn transform_list<T: Send + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	content: impl Node<Context<'static>, Output = List<T>>,
	translation: Item<DVec2>,
	rotation: Item<f64>,
	scale: Item<DVec2>,
	#[hard(-89.9..89.9)] skew: Item<DVec2>,
) -> Result<List<T>, Interrupt> {
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

	let mut list = content.eval(ctx.into_context())?;

	list.left_apply_transform(&matrix);

	Ok(list)
}

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Transform"), icon("NodeResetTransform"))]
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
#[node_macro::node(category("Transform"), icon("NodeReplaceTransform"))]
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
#[node_macro::node(category("Transform"), icon("NodeExtractTransform"), path(core_types::vector))]
fn extract_transform<T: Send>(_: impl Ctx, #[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Artboard)] content: Item<T>) -> Item<DAffine2> {
	Item::new_from_element(content.attribute_cloned_or_default(ATTR_TRANSFORM))
}

#[derive(Debug, thiserror::Error)]
pub enum InvertTransformError {
	#[error("The transform can't be inverted because it collapses to a line or point")]
	Singular,
	#[error("The transform can't be inverted because it has values that aren't finite")]
	NotFinite,
}

/// Produces the inverse of the input transform, which is the transform that undoes the effect of the original transform.
#[node_macro::node(category("Transform"), icon("NodeInvertTransform"))]
fn invert_transform(_: impl Ctx, transform: Item<DAffine2>) -> Result<Item<DAffine2>, (Item<DAffine2>, InvertTransformError)> {
	let element = *transform.element();
	if !element.is_finite() {
		return Err((transform, InvertTransformError::NotFinite));
	}
	let Some(result) = element.matrix2.determinant().is_normal().then(|| element.inverse()).filter(DAffine2::is_finite) else {
		return Err((transform, InvertTransformError::Singular));
	};

	let (_, attributes) = transform.into_parts();
	Ok(Item::from_parts(result, attributes))
}

/// Extracts the translation component from the input transform.
#[node_macro::node(category("Transform"), icon("NodeDecomposeTranslation"))]
fn decompose_translation(_: impl Ctx, transform: Item<DAffine2>) -> Item<DVec2> {
	Item::new_from_element(transform.into_element().translation)
}

/// Extracts the rotation component (in degrees) from the input transform.
#[node_macro::node(category("Transform"), icon("NodeDecomposeRotation"))]
fn decompose_rotation(_: impl Ctx, transform: Item<DAffine2>) -> Item<f64> {
	Item::new_from_element(transform.into_element().decompose_rotation().to_degrees())
}

/// Extracts the scale component from the input transform.
/// **Magnitude** returns the visual length of each axis (always positive, includes any skew contribution).
/// **Pure** returns the isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
#[node_macro::node(category("Transform"), icon("NodeDecomposeScale"))]
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
#[node_macro::node(category("Transform"), icon("NodeDecomposeSkew"))]
fn decompose_skew(_: impl Ctx, transform: Item<DAffine2>) -> Item<f64> {
	Item::new_from_element(transform.into_element().decompose_skew().atan().to_degrees())
}

/// Rotates every item of the content about its origin by its own angle, selected from along a chosen range.
#[node_macro::node(category("Transform"), icon("NodeVaryRotations"))]
fn vary_rotations<T: Send>(
	_: impl Ctx,
	/// The content with items to rotate individually.
	#[implementations(List<Graphic>, List<Vector>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<String>)]
	content: List<T>,
	/// The lower end of the range of angles given to the items.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(-180.)]
	min: Item<f64>,
	/// The upper end of the range of angles given to the items.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(180.)]
	max: Item<f64>,
	/// Where in the range the median angle falls, from its lower end (0) to its upper end (1). The default of 0.5 spreads the angles uniformly, while other values favor angles nearer one end.
	#[range]
	#[hard(0..1)]
	#[default(0.5)]
	bias: Item<f64>,
	/// Whether to reverse the range.
	reverse: Item<bool>,
	/// Whether to randomize the angle selection for each item from throughout the range.
	#[default(true)]
	randomize: Item<bool>,
	/// Seed to determine unique variations on the randomized angle selection.
	#[widget(ParsedWidgetOverride::Custom = "vary_rotations_seed")]
	#[hard(0..)]
	seed: Item<i64>,
	/// The number of items to span across the range before repeating. A 0 value will span the entire range once.
	#[widget(ParsedWidgetOverride::Custom = "vary_rotations_repeat_every")]
	#[hard(0..)]
	repeat_every: Item<i64>,
) -> List<T> {
	let (min, max, bias, reverse) = (min.into_element(), max.into_element(), bias.into_element(), reverse.into_element());
	let (randomize, seed, repeat_every) = (randomize.into_element(), seed.into_element(), repeat_every.into_element());

	let mut content = content;
	let factors = item_factors(content.len(), randomize, seed, repeat_every, reverse);

	for (transform, factor) in content.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM).zip(factors) {
		let angle = min + (max - min) * apply_midpoint_inverse(factor, bias);

		// Turning only the linear part pivots each item about its own origin, in the axes of its parent
		transform.matrix2 = DMat2::from_angle(angle.to_radians()) * transform.matrix2;
	}

	content
}

/// Scales every item of the content about its origin by its own factor, selected from along a chosen range.
#[node_macro::node(category("Transform"), icon("NodeVaryScales"))]
fn vary_scales<T: Send>(
	_: impl Ctx,
	/// The content with items to scale individually.
	#[implementations(List<Graphic>, List<Vector>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<String>)]
	content: List<T>,
	/// The lower end of the range of scale factors given to the items.
	#[unit("x")]
	#[range]
	#[soft(0..2)]
	#[default(0.5)]
	min: Item<f64>,
	/// The upper end of the range of scale factors given to the items.
	#[unit("x")]
	#[range]
	#[soft(0..2)]
	#[default(2.)]
	max: Item<f64>,
	/// Where in the range the median scale factor falls, from its lower end (0) to its upper end (1). The default of 0.5 spreads the scale factors uniformly, while other values favor scale factors nearer one end.
	#[range]
	#[hard(0..1)]
	#[default(0.5)]
	bias: Item<f64>,
	/// Whether to reverse the range.
	reverse: Item<bool>,
	/// Whether to randomize the scale factor selection for each item from throughout the range.
	#[default(true)]
	randomize: Item<bool>,
	/// Seed to determine unique variations on the randomized scale factor selection.
	#[widget(ParsedWidgetOverride::Custom = "vary_scales_seed")]
	#[hard(0..)]
	seed: Item<i64>,
	/// The number of items to span across the range before repeating. A 0 value will span the entire range once.
	#[widget(ParsedWidgetOverride::Custom = "vary_scales_repeat_every")]
	#[hard(0..)]
	repeat_every: Item<i64>,
) -> List<T> {
	let (min, max, bias, reverse) = (min.into_element(), max.into_element(), bias.into_element(), reverse.into_element());
	let (randomize, seed, repeat_every) = (randomize.into_element(), seed.into_element(), repeat_every.into_element());

	let mut content = content;
	let factors = item_factors(content.len(), randomize, seed, repeat_every, reverse);

	for (transform, factor) in content.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM).zip(factors) {
		let scale = min + (max - min) * apply_midpoint_inverse(factor, bias);

		transform.matrix2 *= scale;
	}

	content
}

/// The spread factor of each item for a vary node, handed out from the last item to the first when reversed.
fn item_factors(count: usize, randomize: bool, seed: i64, repeat_every: i64, reverse: bool) -> Vec<f64> {
	let mut factors: Vec<f64> = spread_factors(count, randomize, seed as u64, repeat_every as u64).collect();
	if reverse {
		factors.reverse();
	}

	factors
}

#[cfg(test)]
mod test {
	use super::*;

	fn colors(transforms: &[DAffine2]) -> List<Color> {
		transforms
			.iter()
			.map(|&transform| Item::new_from_element(Color::RED).with_attribute(ATTR_TRANSFORM, transform))
			.collect()
	}

	fn transforms<T>(content: &List<T>) -> Vec<DAffine2> {
		(0..content.len()).map(|index| content.attribute_cloned_or_default(ATTR_TRANSFORM, index)).collect()
	}

	fn item<T>(element: T) -> Item<T> {
		Item::new_from_element(element)
	}

	fn rotate(content: List<Color>, min: f64, max: f64, bias: f64, reverse: bool) -> List<Color> {
		vary_rotations((), content, item(min), item(max), item(bias), item(reverse), item(false), item(0), item(0))
	}

	fn scale(content: List<Color>, min: f64, max: f64, bias: f64, randomize: bool) -> List<Color> {
		vary_scales((), content, item(min), item(max), item(bias), item(false), item(randomize), item(0), item(0))
	}

	fn angles(content: &List<Color>) -> Vec<f64> {
		transforms(content).iter().map(|transform| transform.decompose_rotation().to_degrees()).collect()
	}

	fn assert_near(actual: &[f64], expected: &[f64]) {
		assert_eq!(actual.len(), expected.len());
		for (actual, expected) in actual.iter().zip(expected) {
			assert!((actual - expected).abs() < 1e-9, "expected {expected:?}, found {actual:?}");
		}
	}

	#[test]
	fn rotations_spread_evenly_across_the_range() {
		let rotated = rotate(colors(&[DAffine2::IDENTITY; 3]), 0., 90., 0.5, false);
		assert_near(&angles(&rotated), &[0., 45., 90.]);

		let reversed = rotate(colors(&[DAffine2::IDENTITY; 3]), 0., 90., 0.5, true);
		assert_near(&angles(&reversed), &[90., 45., 0.]);
	}

	#[test]
	fn reverse_runs_the_randomized_pattern_from_the_other_end() {
		let randomized = |reverse| {
			angles(&vary_rotations(
				(),
				colors(&[DAffine2::IDENTITY; 5]),
				item(0.),
				item(90.),
				item(0.3),
				item(reverse),
				item(true),
				item(7),
				item(0),
			))
		};

		let mut forward = randomized(false);
		forward.reverse();
		assert_near(&randomized(true), &forward);
	}

	#[test]
	fn bias_places_the_middle_item() {
		let rotated = rotate(colors(&[DAffine2::IDENTITY; 3]), 0., 100., 0.2, false);
		assert_near(&angles(&rotated), &[0., 20., 100.]);
	}

	#[test]
	fn rotation_pivots_about_the_item_origin_without_shearing() {
		let original = DAffine2::from_translation(DVec2::new(5., 7.)) * DAffine2::from_scale(DVec2::new(2., 1.));
		let rotated = transforms(&rotate(colors(&[original]), 90., 90., 0.5, false))[0];

		assert_eq!(rotated.translation, original.translation);
		assert!(rotated.matrix2.x_axis.abs_diff_eq(DVec2::new(0., 2.), 1e-9));
		assert!(rotated.matrix2.y_axis.abs_diff_eq(DVec2::new(-1., 0.), 1e-9));
	}

	#[test]
	fn scales_keep_the_item_origin() {
		let original = DAffine2::from_translation(DVec2::new(5., 7.));
		let scaled = transforms(&scale(colors(&[original; 2]), 1., 3., 0.5, false));

		assert_eq!(scaled[0], original);
		assert_eq!(scaled[1], DAffine2::from_scale_angle_translation(DVec2::splat(3.), 0., original.translation));
	}

	#[test]
	fn extreme_biases_stay_within_the_range() {
		for bias in [0., 1.] {
			for transform in transforms(&scale(colors(&[DAffine2::IDENTITY; 64]), 1., 2., bias, true)) {
				let factor = transform.matrix2.x_axis.length();
				assert!((1. ..=2.).contains(&factor), "bias {bias} gave scale {factor}");
			}
		}
	}
}
