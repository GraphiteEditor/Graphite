//! The history properties of `migration_runner::testing` on the examples.

use crate::examples::{AngleBetweenDegrees, Axis, ExtractXyToSplit, OpacityToggles, Tagged, TransformDegrees};
use migration_core::{decl, literal};
use migration_runner::testing::{Random, SourceContract, check_commutation, check_undo, registered};

fn numbers(input_count: usize) -> SourceContract {
	SourceContract::new(input_count, |random, _| literal(Tagged::Number(random.below(10) as f64)))
}

#[test]
fn rename_commutes() {
	check_commutation(registered(decl!("graphene_core::ops::AddNode"@1)), &numbers(2));
}

#[test]
fn opacity_toggles_commute() {
	check_commutation(&OpacityToggles, &numbers(2));
}

#[test]
fn transform_degrees_commutes() {
	check_commutation(&TransformDegrees, &numbers(3));
}

#[test]
fn angle_between_degrees_commutes() {
	check_commutation(&AngleBetweenDegrees, &numbers(2));
}

#[test]
fn extract_xy_to_split_commutes() {
	let contract = SourceContract {
		// The axis only accepts what produces an axis, which no extract XY output does
		reads_own_output: |slot| slot != 1,
		..SourceContract::new(2, |random: &mut Random, slot| match slot {
			1 => literal(Tagged::XY(random.pick(&[Axis::X, Axis::Y]))),
			_ => literal(Tagged::Number(random.below(10) as f64)),
		})
	};
	check_commutation(&ExtractXyToSplit, &contract);
}

#[test]
fn undo_through_rewritten_history_matches_migrated_undo() {
	check_undo(&TransformDegrees, &numbers(3));
}
