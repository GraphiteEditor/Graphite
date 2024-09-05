mod aabb;
mod epsilons;
mod intersection_path_segment;
mod line_segment;
mod line_segment_aabb;
mod math;
mod path;
mod path_boolean;
mod path_command;
mod path_cubic_segment_self_intersection;
mod path_data;
mod path_segment;
mod quad_tree;
mod vector;
#[cfg(test)]
mod visual_tests;

pub use path_boolean::{path_boolean, FillRule, PathBooleanOperation};
pub use path_data::{path_from_path_data, path_to_path_data};

#[cfg(test)]
mod test {
	use crate::{
		path_boolean::{self, FillRule, PathBooleanOperation},
		path_data::{path_from_path_data, path_to_path_data},
	};
	use path_boolean::path_boolean;

	#[test]
	fn square() {
		// let a = path_from_path_data(
		//     "M 39,20 A 19,19 0 0 1 20,39 19,19 0 0 1 1,20 19,19 0 0 1 20,1 19,19 0 0 1 39,20 Z",
		// );
		// let b = path_from_path_data(
		//     "M 47,28 A 19,19 0 0 1 28,47 19,19 0 0 1 9,28 19,19 0 0 1 28,9 19,19 0 0 1 47,28 Z",
		// );
		let a = path_from_path_data("M 10 10 L 50 10 L 30 40 Z");
		let b = path_from_path_data("M 20 30 L 60 30 L 60 50 L 20 50 Z");
		// let a = path_from_path_data("M 0 0 L 10 0 L 5 10 Z");
		// let b = path_from_path_data("M 0 5 L 10  5 L 5 15  Z");
		let union = path_boolean(
			&a,
			path_boolean::FillRule::NonZero,
			&b,
			path_boolean::FillRule::NonZero,
			path_boolean::PathBooleanOperation::Intersection,
		)
		.unwrap();
		dbg!(path_to_path_data(&union[0], 0.001));
		// panic!();
	}

	#[test]
	fn nesting_01() {
		let a = path_from_path_data("M 47,24 A 23,23 0 0 1 24,47 23,23 0 0 1 1,24 23,23 0 0 1 24,1 23,23 0 0 1 47,24 Z");
		let b = path_from_path_data(
			"M 37.909023,24 A 13.909023,13.909023 0 0 1 24,37.909023 13.909023,13.909023 0 0 1 10.090978,24 13.909023,13.909023 0 0 1 24,10.090978 13.909023,13.909023 0 0 1 37.909023,24 Z",
		);
		let union = path_boolean(&a, path_boolean::FillRule::NonZero, &b, path_boolean::FillRule::NonZero, path_boolean::PathBooleanOperation::Union).unwrap();
		dbg!(path_to_path_data(&union[0], 0.001));
	}
	#[test]
	fn nesting_02() {
		let a = path_from_path_data("M 0.99999994,31.334457 C 122.61195,71.81859 -79.025816,-5.5803326 47,32.253367 V 46.999996 H 0.99999994 Z");
		let b = path_from_path_data("m 25.797222,29.08718 c 0,1.292706 -1.047946,2.340652 -2.340652,2.340652 -1.292707,0 -2.340652,-1.047946 -2.340652,-2.340652 0,-1.292707 1.047945,-2.340652 2.340652,-2.340652 1.292706,0 2.340652,1.047945 2.340652,2.340652 z M 7.5851073,28.332212 c 1e-7,1.292706 -1.0479456,2.340652 -2.3406521,2.340652 -1.2927063,-1e-6 -2.3406518,-1.047946 -2.3406517,-2.340652 -10e-8,-1.292707 1.0479454,-2.340652 2.3406517,-2.340652 1.2927065,-1e-6 2.3406522,1.047945 2.3406521,2.340652 z");

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
	}
	#[test]
	fn simple_07() {
		let a = path_from_path_data("M 37.671452,24 C 52.46888,31.142429 42.887716,37.358779 24,37.671452 16.4505,37.796429 10.328548,31.550534 10.328548,24 c 0,-7.550534 6.120918,-13.671452 13.671452,-13.671452 7.550534,0 6.871598,10.389295 13.671452,13.671452 z",
    );
		let b = path_from_path_data("M 37.671452,24 C 33.698699,53.634887 29.50935,49.018306 24,37.671452 20.7021,30.879219 10.328548,31.550534 10.328548,24 c 0,-7.550534 6.120918,-13.671452 13.671452,-13.671452 7.550534,0 14.674677,6.187863 13.671452,13.671452 z");

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
	}
	#[test]
	fn rect_ellipse() {
		let a = path_from_path_data("M 0,-10 C5.517848,-10 10,-5.517848 10,0 C10,5.517848 5.517848,10.000000 0.000000,10.000000 C-5.517848,10 -10,5.517848 -10,0 C-10,-5.517848 -5.517848,-10 0,-10");
		let b = path_from_path_data(
			"M-10,-10.438833C-10,-10.438833 10,-10.438833 10,-10.438833 C10,-10.438833 10,5.770318 10,5.770318 C10,5.770318 -10,5.770318 -10,5.770318 C-10,5.770318 -10,-10.438833 -10,-10.438833 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
	}
	#[test]
	fn red_dress_loop() {
		let a = path_from_path_data("M969.000000,0.000000C969.000000,0.000000 1110.066898,76.934393 1085.000000,181.000000 C1052.000000,318.000000 1199.180581,334.301571 1277.000000,319.000000 C1455.000000,284.000000 1586.999985,81.000000 1418.000000,0.000000 C1418.000000,0.000000 969.000000,0.000000 969.000000,0.000000");
		let b = path_from_path_data(
			"M1211.000000,0.000000C1211.000000,0.000000 1255.000000,78.000000 1536.000000,95.000000 C1536.000000,95.000000 1536.000000,0.000000 1536.000000,0.000000 C1536.000000,0.000000 1211.000000,0.000000 1211.000000,0.000000 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Intersection).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
	}
}
