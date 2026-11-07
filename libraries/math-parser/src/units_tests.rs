use crate::ast::Node;
use crate::context::{EvalContext, NothingMap, ValueProvider};
use crate::object::Object;
use crate::quantity::Quantity;
use crate::units::{Dimension, Unit};
use crate::value::Value;

const EPSILON: f64 = 1e-9;

fn evaluate_with(source: &str, pixels_per_inch: f64) -> Result<Object, String> {
	let node = Node::try_parse_from_str(source).map_err(|error| error.to_string())?;
	node.eval(&EvalContext::new(NothingMap, NothingMap).with_pixels_per_inch(pixels_per_inch))
		.map_err(|error| error.to_string())
}

fn quantity(source: &str) -> Quantity {
	match evaluate_with(source, 96.) {
		Ok(Object::Value(quantity)) => quantity,
		result => panic!("`{source}` should be a value, not {result:?}"),
	}
}

fn shown(source: &str) -> String {
	quantity(source).to_string()
}

fn error(source: &str) -> String {
	match evaluate_with(source, 96.) {
		Err(error) => error,
		Ok(result) => panic!("`{source}` should fail, not give {result}"),
	}
}

fn unit(name: &str) -> Unit {
	Unit::named(name).unwrap_or_else(|| panic!("`{name}` should name a unit"))
}

/// The value read in the given unit.
fn read(source: &str, unit_name: &str) -> f64 {
	let quantity = quantity(source);
	quantity
		.convert(unit(unit_name), 96.)
		.unwrap_or_else(|error| panic!("`{source}` should convert to `{unit_name}`: {error:?}"))
		.as_real()
		.unwrap()
}

fn assert_close(actual: f64, expected: f64, source: &str) {
	assert!((actual - expected).abs() <= EPSILON * expected.abs().max(1.), "`{source}` gave {actual}, expected {expected}");
}

#[test]
fn a_number_before_a_unit_measures_in_it() {
	assert_eq!(shown("5mm"), "5 mm");
	assert_eq!(shown("5 mm"), "5 mm");
	assert_eq!(shown("2.5 inches"), "2.5 in");
	assert_eq!(shown("3 feet"), "3 ft");
	assert_eq!(shown("90 degrees"), "90°");
	assert_eq!(shown("45deg"), "45°");
	assert_eq!(shown("45°"), "45°");
	assert_eq!(shown("50%"), "50%");
	assert_eq!(shown("12.5 %"), "12.5%");
	assert_eq!(shown("5Hz"), "5 Hz");
	assert_eq!(shown("2 kHz"), "2 kHz");
	assert_eq!(shown("1000um"), "1000 µm");
	assert_eq!(shown("1000µm"), "1000 µm");
	assert_eq!(shown("9.8m/s^2"), "9.8 m/s^2");
	assert_eq!(shown("-5mm"), "-5 mm");
}

#[test]
fn sums_take_the_unit_that_converts_exactly() {
	assert_eq!(shown("5m + 3m"), "8 m");
	assert_eq!(shown("5ft + 3ft"), "8 ft");
	assert_eq!(shown("1m + 1mm"), "1001 mm");
	assert_eq!(shown("1ft + 1in"), "13 in");
	assert_eq!(shown("90deg + 0.25turn"), "180°");
	assert_eq!(shown("1in + 1px"), "97 px");
	assert_close(read("5m + 3ft", "m"), 5.9144, "5m + 3ft");
	assert_eq!(quantity("5m + 3ft").unit, unit("m"));
	assert_close(read("1in + 1mm", "mm"), 26.4, "1in + 1mm");
	assert_eq!(quantity("1in + 1mm").unit, unit("mm"));

	// Radians are coherent, so a mix with them keeps them
	assert_eq!(quantity("asin(0.5) + 30deg").unit, unit("rad"));
	assert_close(read("asin(0.5) + 30deg", "rad"), std::f64::consts::PI / 3., "asin(0.5) + 30deg");
}

#[test]
fn a_plain_number_only_adds_to_a_unit_when_it_is_zero() {
	assert!(error("1in + 5").contains("A plain number can't be added to a value in `in`"));
	assert!(error("5 - 1in").contains("A plain number can't be subtracted from a value in `in`"));
	assert!(error("1 + 50%").contains("plain number"));
	assert!(error("asin(0.5) + 1").contains("A plain number can't be added to a value in `rad`"));
	assert_eq!(shown("0 + 5mm"), "5 mm");
	assert_eq!(shown("5mm - 0"), "5 mm");
	assert_eq!(read("max(0, 3mm - 5mm)", "mm"), 0.);
	assert_eq!(evaluate_with("0 < 1mm", 96.).unwrap().as_bool(), Some(true));
}

#[test]
fn different_dimensions_never_mix() {
	assert!(error("1mm + 1s").contains("measure different things"));
	assert!(error("1mm < 1s").contains("measure different things"));
	assert!(error("max(1mm, 1kg)").contains("measure different things"));
	assert!(error("1mm == 1s").contains("measure different things"));
}

#[test]
fn a_number_and_unit_after_a_value_with_a_unit_is_the_divisor() {
	assert_eq!(quantity("10mm/2mm").value, Value::from_i64(5));
	assert!(quantity("10mm/2mm").is_plain());
	assert_eq!(shown("100km/2h"), "50 km/h");
	assert_eq!(shown("10mm / 2 mm"), "5");
	assert_eq!(shown("1in/2.54cm"), "1");
	assert_eq!(quantity("10mm/2mm^2").unit, unit("mm").inverse());
	assert_eq!(shown("3mm * 2 / 4mm"), "1.5");

	// The dividend's value decides, however its name is spelled
	assert_eq!(shown("a / 2mm where a = 10mm"), "5");
	assert_eq!(shown("x / 2mm where x = 3"), "1.5 mm");

	// Without a unit in the dividend, a unit after the number applies to the whole quotient
	assert_eq!(shown("3/4in"), "0.75 in");
	assert_close(read("pi/2 rad", "rad"), std::f64::consts::FRAC_PI_2, "pi/2 rad");
	assert_close(quantity("1/2pi").value.as_real().unwrap(), std::f64::consts::FRAC_PI_2, "1/2pi");
	assert_eq!(shown("50% / 2mm"), "0.25 mm");

	// A name defined as a variable is no unit, so it multiplies left to right like any other factor
	assert_eq!(shown("s/2m where s = 12, m = 3"), "18");
	assert_eq!(shown("a/2m where a = 12mm, m = 3"), "18 mm");
	let node = Node::try_parse_from_str("10mm / 2m").unwrap();
	let host = crate::context::ValueMap([("m".to_string(), Value::from_i64(3))].into_iter().collect());
	assert_eq!(node.eval(&EvalContext::new(host, NothingMap)).unwrap().to_string(), "15 mm");

	// Only juxtaposition joins the divisor
	assert_eq!(shown("10mm/2*mm"), "5 mm^2");

	// A matrix's units are known only once evaluated, so it divides by the whole measure, unless one of its names is a variable
	assert_eq!(shown("(0mm..10mm) / 2mm"), "5");
	assert_eq!(shown("(0..12) / 2m where m = 3"), "18");
}

#[test]
fn feet_then_inches_sum() {
	assert_eq!(shown("5'6\""), "66 in");
	assert_eq!(shown("5'"), "5 ft");
	assert_eq!(shown("6\""), "6 in");
}

#[test]
fn percentages_scale_and_stay_percentages_beside_plain_numbers() {
	assert_eq!(shown("50% + 10%"), "60%");
	assert_eq!(shown("50% / 2"), "25%");
	assert_eq!(shown("2 * 50%"), "100%");
	assert_eq!(shown("50% * 50%"), "25%");
	assert_eq!(shown("50% * 10mm"), "5 mm");
	assert_eq!(quantity("100 / (50%)").value, Value::from_i64(200));
	assert_eq!(quantity("x / 50% where x = 3").value, Value::from_i64(6));
	assert_eq!(quantity("50% / 25%").value, Value::from_i64(2));
	assert_eq!(evaluate_with("50%", 96.).unwrap().as_real(), Some(0.5));
	assert!(evaluate_with("7 % 3", 96.).is_err());
}

#[test]
fn products_merge_units_of_one_dimension() {
	assert_eq!(shown("3mm * 2mm"), "6 mm^2");
	assert_eq!(quantity("3mm * 2in").unit, unit("mm").power(2.).unwrap());
	assert_close(quantity("3mm * 2in").value.as_real().unwrap(), 152.4, "3mm * 2in");
}

#[test]
fn powers_and_roots_raise_units() {
	assert_eq!(shown("(2mm)^2"), "4 mm^2");
	assert_eq!(shown("sqrt(9mm^2)"), "3 mm");
	assert_eq!(shown("cbrt(8mm^3)"), "2 mm");
	assert_eq!(shown("root(16mm^4, 4)"), "2 mm");
	assert_eq!(shown("(4mm^2)^0.5"), "2 mm");
	assert_eq!(shown("(2mm)^0"), "1");
	assert!(error("sqrt(1mm)").contains("fractional exponent"));
	assert!(error("2^(1mm)").contains("An exponent can't be in `mm`"));
}

#[test]
fn circular_functions_take_angles_or_radians() {
	assert_close(quantity("sin(30deg)").value.as_real().unwrap(), 0.5, "sin(30deg)");
	assert_close(quantity("sin(pi/6)").value.as_real().unwrap(), 0.5, "sin(pi/6)");
	assert_close(quantity("cos(0.5turn)").value.as_real().unwrap(), -1., "cos(0.5turn)");
	assert!(error("sin(1mm)").contains("`sin` takes an angle or a plain number"));
	// The inverse functions give angles in radians, which the circular functions read back
	assert_eq!(quantity("asin(1)").unit, unit("rad"));
	assert_close(read("asin(1)", "deg"), 90., "asin(1)");
	assert_close(quantity("sin(asin(0.5))").value.as_real().unwrap(), 0.5, "sin(asin(0.5))");
	assert_close(read("atan2(1, 1)", "deg"), 45., "atan2(1, 1)");
	assert_close(quantity("atan2(1, 1) / turn + 0.5").value.as_real().unwrap(), 0.625, "atan2(1, 1) / turn + 0.5");
	assert!(error("atan2(1, 1) / (2pi) + 0.5").contains("A plain number can't be added to a value in `rad`"));
	assert_eq!(quantity("atan2(3mm, 4mm)").unit, unit("rad"));
	assert_close(read("atan2(3mm, 4cm)", "rad"), (3_f64).atan2(40.), "atan2(3mm, 4cm)");
	assert_eq!(quantity("angle(i, j)").unit, unit("rad"));
	assert_close(read("angle(1mm i, 1mm j)", "deg"), 90., "angle(1mm i, 1mm j)");

	// Dividing by `rad` reads an angle as plain radians
	assert_close(quantity("asin(1)/rad + 1").value.as_real().unwrap(), std::f64::consts::FRAC_PI_2 + 1., "asin(1)/rad + 1");

	// An exponent with no plain part rotates, read in radians
	let rotation = quantity("e^(i 90deg)").number().to_quaternion();
	assert!(rotation.w.abs() < EPSILON && (rotation.x - 1.).abs() < EPSILON);
	assert!(evaluate_with("e^(2 + i 90deg)", 96.).is_err());
	assert!(evaluate_with("exp(i 90deg)", 96.).is_ok());
}

#[test]
fn rounding_takes_plain_numbers_and_snap_rounds_in_a_unit() {
	assert!(error("floor(1.5mm)").contains("snap(x, 1 mm)"));
	assert!(error("round(2.5in)").contains("snap(x, 1 in)"));
	assert_eq!(shown("snap(13mm, 5mm)"), "15 mm");
	assert_eq!(shown("mod(13mm, 5mm)"), "3 mm");
	assert_eq!(shown("floor(150%)"), "1");
}

#[test]
fn builtins_carry_units_by_their_rules() {
	assert_eq!(shown("max(1mm, 1in)"), "25.4 mm");
	assert_eq!(shown("min(1cm, 1mm)"), "1 mm");
	assert_eq!(shown("abs(-3mm)"), "3 mm");
	assert_eq!(shown("sign(-3mm)"), "-1");
	assert_eq!(shown("hypot(3mm, 4mm)"), "5 mm");
	assert_eq!(shown("mean(1mm, 3mm)"), "2 mm");
	assert_eq!(quantity("variance(1mm, 3mm)").unit, unit("mm").power(2.).unwrap());
	assert_eq!(shown("lerp(0mm, 10mm, 50%)"), "5 mm");
	assert_eq!(shown("count(1mm, 2s)"), "2");
	assert_eq!(shown("|3mm i + 4mm j|"), "5 mm");
	assert!(quantity("normalize(3mm i + 4mm j)").is_plain());
	assert_eq!(shown("dot(1mm i, 2mm i)"), "2 mm^2");
	assert_eq!(quantity("distance(1mm i, 4mm i)").unit, unit("mm"));
	let rotated = quantity("rotate(1mm i, 90deg)");
	assert_eq!(rotated.unit, unit("mm"));
	assert!((rotated.number().to_quaternion().y - 1.).abs() < EPSILON);
	assert!(error("ln(2mm)").contains("`ln` takes plain numbers"));
	assert!(error("5mm!").contains("`!` takes plain numbers"));
	assert!(evaluate_with("1mm && 1", 96.).is_err());
}

#[test]
fn angles_and_cycles_divide_out_explicitly() {
	assert_eq!(quantity("720deg / turn").value, Value::from_i64(2));
	assert_eq!(quantity("720deg / deg").value, Value::from_i64(720));
	assert_close(quantity("720deg / rad").value.as_real().unwrap(), 4. * std::f64::consts::PI, "720deg / rad");
	assert_eq!(quantity("60rpm / turn").dimension(), quantity("1Hz").dimension());
	assert_close(read("60rpm / turn", "Hz"), 1., "60rpm / turn");
	assert_close(read("1rpm * 1s", "turn"), 1. / 60., "1rpm * 1s");
	assert_eq!(quantity("1Hz * 1min").value, Value::from_i64(60));
	assert_eq!(quantity("2px * 3deg").dimension(), Dimension([1, 0, 0, 1]));
}

#[test]
fn pixels_are_sized_by_the_pixels_per_inch() {
	assert_eq!(read("1in", "px"), 96.);
	assert_close(read("10mm", "px"), 10. / 25.4 * 96., "10mm");
	assert_eq!(read("72pt", "px"), 96.);
	let at_300 = match evaluate_with("1in", 300.).unwrap() {
		Object::Value(quantity) => quantity.convert(unit("px"), 300.).unwrap().as_real().unwrap(),
		_ => unreachable!(),
	};
	assert_eq!(at_300, 300.);
	assert_eq!(shown("1in + 1px"), "97 px");
}

#[test]
fn a_field_reads_a_plain_result_in_its_own_unit() {
	let px = unit("px");
	assert_eq!(quantity("5").read_in(px, 96.).unwrap().as_real(), Some(5.));
	assert_eq!(quantity("1in").read_in(px, 96.).unwrap().as_real(), Some(96.));
	assert!(quantity("1s").read_in(px, 96.).is_err());
}

#[test]
fn hosts_read_angles_as_radians_and_lengths_not_at_all() {
	assert_close(evaluate_with("90deg", 96.).unwrap().as_real().unwrap(), std::f64::consts::FRAC_PI_2, "90deg");
	assert_eq!(evaluate_with("5mm", 96.).unwrap().as_real(), None);
	assert_eq!(evaluate_with("5", 96.).unwrap().as_real(), Some(5.));
}

struct Bindings;

impl ValueProvider for Bindings {
	fn get_value(&self, name: &str) -> Option<Value> {
		(name == "m").then(|| Value::from_i64(3))
	}

	fn get_quantity(&self, name: &str) -> Option<Quantity> {
		match name {
			"x" => Some(Quantity::new(Value::from_i64(10), Unit::named("px")?)),
			_ => self.get_value(name).map(Quantity::plain),
		}
	}
}

#[test]
fn bindings_shadow_units_and_carry_their_own() {
	let evaluate = |source: &str| Node::try_parse_from_str(source).unwrap().eval(&EvalContext::new(Bindings, NothingMap)).unwrap();
	assert_eq!(evaluate("2m").as_real(), Some(6.));
	assert_eq!(evaluate("2\\m").as_quantity().unwrap().unit, unit("m"));
	assert_eq!(evaluate("x + 1in").to_string(), "106 px");
	assert_eq!(evaluate("x * 2").to_string(), "20 px");
	assert_eq!(evaluate("a where a = 5mm").to_string(), "5 mm");
	assert_eq!(evaluate("f(2mm) where f(t) = t * 2").to_string(), "4 mm");
	assert_eq!(evaluate("{1mm if 1 > 0, 2mm otherwise}").to_string(), "1 mm");
}

#[test]
fn ranges_and_matrices_carry_units() {
	assert_eq!(shown("clamp(15mm, 0..10mm)"), "10 mm");
	assert_close(read("clamp(5mm, 0..1cm)", "mm"), 5., "clamp(5mm, 0..1cm)");
	assert_eq!(shown("remap(5mm, 0..10mm, 0..1)"), "0.5");
	assert_eq!(shown("remap(0.5, 0..1, 0..10mm)"), "5 mm");
	assert_eq!(evaluate_with("within(5mm, 0..1cm)", 96.).unwrap().as_bool(), Some(true));
	assert_eq!(shown("(0..10mm) 0.5"), "5 mm");
	assert_eq!(shown("(0..10mm)^-1 (5mm)"), "0.5");
	assert_eq!(shown("(I + 5mm i) (1mm i)"), "(6i) mm");
	assert_eq!(quantity("det(0..(2mm i + 3mm j))").unit, unit("mm").power(2.).unwrap());
	assert!(error("clamp(5, 0..10mm)").contains("plain number"));

	let rotated = quantity("rotation(90deg) (1mm i)");
	assert_eq!(rotated.unit, unit("mm"));
	assert!((rotated.number().to_quaternion().y - 1.).abs() < EPSILON);
}

#[test]
fn exponents_stay_in_range() {
	assert_eq!(quantity("1 / (1m^-127)").unit, unit("m").power(127.).unwrap());
	assert!(evaluate_with("1m^-128", 96.).is_err());
	assert!(evaluate_with("1m^200", 96.).is_err());
	assert_eq!(shown("root(1m^49, 49)"), "1 m");
}

#[test]
fn exact_conversions_survive_large_operands() {
	assert_eq!(quantity("2540000000000000127mm / in").value, Value::from_i64(100_000_000_000_000_005));
	assert_close(quantity("1e308m / (1e308cm)").value.as_real().unwrap(), 100., "1e308m / (1e308cm)");
	assert_close(read("50% * (1e308mm)", "mm"), 5e307, "50% * (1e308mm)");
}

#[test]
fn matrices_with_units_apply_compose_and_compare() {
	assert_eq!(shown("(1mm I + 1) 0"), "1");
	assert_eq!(shown("((1mm..2mm)^1) 0.5"), "1.5 mm");
	assert_eq!(evaluate_with("within(i, (0..10mm)^0)", 96.).unwrap().as_bool(), Some(false));
	assert_eq!(evaluate_with("[1m] != [2m] != [1001mm]", 96.).unwrap().as_bool(), Some(true));
	assert_eq!(evaluate_with("[1m] == [1000mm]", 96.).unwrap().as_bool(), Some(true));
	assert_eq!(shown("shear(1m i, 1m j, 1) j"), shown("shear(100cm i, 100cm j, 1) j"));
	assert_eq!(shown("scale(50%) 1"), shown("scale(0.5) 1"));
}

#[test]
fn a_sign_before_feet_and_inches_applies_to_both() {
	assert_eq!(shown("-5'6\""), "-66 in");
	assert_eq!(shown("1ft - 5'6\""), "-54 in");
}

#[test]
fn equal_ranges_in_different_units_compare_equal() {
	assert_eq!(evaluate_with("(0..1cm) == (0..10mm)", 96.).unwrap().as_bool(), Some(true));
	assert_eq!(evaluate_with("(0..1in) != (0..1cm) != (0..10mm)", 96.).unwrap().as_bool(), Some(false));
	// A part outside a range's axes passes through untouched
	assert_eq!(quantity("((0..1in) (0..1cm)) (0.5 + 2j)").number().to_quaternion().y, 2.);
}

#[test]
fn percentages_stand_for_their_fraction_where_plain_numbers_go() {
	assert_eq!(shown("clamp(150%, 0..1)"), "1");
	assert_eq!(shown("remap(50%, 0..1, 0..10mm)"), "5 mm");
	assert_eq!(evaluate_with("within(50%, 0..1)", 96.).unwrap().as_bool(), Some(true));
	assert_eq!(quantity("100 / 50%").value, Value::from_i64(200));
}

#[test]
fn inches_convert_to_pixels_exactly_at_any_whole_resolution() {
	for pixels_per_inch in 1..=4000 {
		let pixels_per_inch = pixels_per_inch as f64;
		let pixels = match evaluate_with("1in", pixels_per_inch).unwrap() {
			Object::Value(quantity) => quantity.convert(unit("px"), pixels_per_inch).unwrap().as_real().unwrap(),
			_ => unreachable!(),
		};
		assert_eq!(pixels, pixels_per_inch);
	}
}

#[test]
fn plain_matrices_are_named_plainly_in_errors() {
	assert!(error("I + (0..10mm)").contains("A plain number can't be added to a value in `mm`"));
}
