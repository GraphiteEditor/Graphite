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

pub use intersection_path_segment::path_segment_intersection;
pub use path_boolean::{path_boolean, FillRule, PathBooleanOperation, EPS};
pub use path_data::{path_from_path_data, path_to_path_data};
pub use path_segment::PathSegment;

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
		assert!(!union[0].is_empty());
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
		assert!(!union[0].is_empty());
	}
	#[test]
	fn nesting_02() {
		let a = path_from_path_data("M 0.99999994,31.334457 C 122.61195,71.81859 -79.025816,-5.5803326 47,32.253367 V 46.999996 H 0.99999994 Z");
		let b = path_from_path_data("m 25.797222,29.08718 c 0,1.292706 -1.047946,2.340652 -2.340652,2.340652 -1.292707,0 -2.340652,-1.047946 -2.340652,-2.340652 0,-1.292707 1.047945,-2.340652 2.340652,-2.340652 1.292706,0 2.340652,1.047945 2.340652,2.340652 z M 7.5851073,28.332212 c 1e-7,1.292706 -1.0479456,2.340652 -2.3406521,2.340652 -1.2927063,-1e-6 -2.3406518,-1.047946 -2.3406517,-2.340652 -10e-8,-1.292707 1.0479454,-2.340652 2.3406517,-2.340652 1.2927065,-1e-6 2.3406522,1.047945 2.3406521,2.340652 z");

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn nesting_03() {
		let a = path_from_path_data("m 21.829117,3.5444345 h 4.341766 V 16.502158 H 21.829117 Z M 47,24 A 23,23 0 0 1 24,47 23,23 0 0 1 1,24 23,23 0 0 1 24,1 23,23 0 0 1 47,24 Z");
		let b = path_from_path_data("M 24 6.4960938 A 17.504802 17.504802 0 0 0 6.4960938 24 A 17.504802 17.504802 0 0 0 24 41.503906 A 17.504802 17.504802 0 0 0 41.503906 24 A 17.504802 17.504802 0 0 0 24 6.4960938 z M 24 12.193359 A 11.805881 11.805881 0 0 1 35.806641 24 A 11.805881 11.805881 0 0 1 24 35.806641 A 11.805881 11.805881 0 0 1 12.193359 24 A 11.805881 11.805881 0 0 1 24 12.193359 z ");

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		// Add more specific assertions about the resulting path if needed
		let path_string = dbg!(path_to_path_data(&result[0], 0.001));
		assert_eq!(path_string.chars().filter(|c| c == &'M').count(), 1, "More than one path returned");
		assert!(!result[0].is_empty());
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
		dbg!(path_to_path_data(&result[0], 0.001));
		assert!(!result[0].is_empty());
	}
	#[test]
	fn rect_ellipse() {
		let a = path_from_path_data("M0,0C0,0 100,0 100,0 C100,0 100,100 100,100 C100,100 0,100 0,100 C0,100 0,0 0,0 Z");
		let b = path_from_path_data("M50,0C77.589239,0 100,22.410761 100,50 C100,77.589239 77.589239,100 50,100 C22.410761,100 0,77.589239 0,50 C0,22.410761 22.410761,0 50,0 Z");

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		assert!(!result[0].is_empty());
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
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn painted_dreams_1() {
		let a = path_from_path_data("M969.000000,0.000000C969.000000,0.000000 1110.066898,76.934393 1085.000000,181.000000 C1052.000000,318.000000 1199.180581,334.301571 1277.000000,319.000000 C1455.000000,284.000000 1586.999985,81.000000 1418.000000,0.000000 C1418.000000,0.000000 969.000000,0.000000 969.000000,0.000000 Z");
		let b = path_from_path_data(
			"M763.000000,0.000000C763.000000,0.000000 1536.000000,0.000000 1536.000000,0.000000 C1536.000000,0.000000 1536.000000,254.000000 1536.000000,254.000000 C1536.000000,254.000000 1462.000000,93.000000 1271.000000,199.000000 C1149.163056,266.616314 976.413656,188.510842 908.000000,134.000000 C839.586344,79.489158 763.000000,0.000000 763.000000,0.000000 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Intersection).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn painted_dreams_2() {
		let a = path_from_path_data("M0,340C161.737914,383.575765 107.564182,490.730587 273,476 C419,463 481.741198,514.692273 481.333333,768 C481.333333,768 -0,768 -0,768 C-0,768 0,340 0,340 Z ");
		let b = path_from_path_data(
			"M458.370270,572.165771C428.525848,486.720093 368.618805,467.485992 273,476 C107.564178,490.730591 161.737915,383.575775 0,340 C0,340 0,689 0,689 C56,700 106.513901,779.342590 188,694.666687 C306.607422,571.416260 372.033966,552.205139 458.370270,572.165771 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Union).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn painted_dreams_3() {
		let a = path_from_path_data("M889,0C889,0 889,21 898,46 C909.595887,78.210796 872.365858,104.085306 869,147 C865,198 915,237 933,273 C951,309 951.703704,335.407407 923,349 C898.996281,360.366922 881,367 902,394 C923,421 928.592593,431.407407 898,468 C912.888889,472.888889 929.333333,513.333333 896,523 C896,523 876,533.333333 886,572 C896.458810,612.440732 873.333333,657.777778 802.666667,656.444444 C738.670245,655.236965 689,643 655,636 C621,629 604,623 585,666 C566,709 564,768 564,768 C564,768 0,768 0,768 C0,768 0,0 0,0 C0,0 889,0 889,0 Z ");
		let b = path_from_path_data(
			"M552,768C552,768 993,768 993,768 C993,768 1068.918039,682.462471 1093,600 C1126,487 1007.352460,357.386071 957,324 C906.647540,290.613929 842,253 740,298 C638,343 491.342038,421.999263 491.342038,506.753005 C491.342038,641.999411 552,768 552,768 Z ",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Difference).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn painted_dreams_4() {
		let a = path_from_path_data("M458.370270,572.165771C372.033966,552.205139 306.607422,571.416260 188.000000,694.666687 C106.513901,779.342590 56.000000,700.000000 0.000000,689.000000 C0.000000,689.000000 0.000000,768.000000 0.000000,768.000000 C0.000000,768.000000 481.333344,768.000000 481.333344,768.000000 C481.474091,680.589417 474.095154,617.186768 458.370270,572.165771 Z ");
		let b = path_from_path_data(
			"M364.000000,768.000000C272.000000,686.000000 294.333333,468.666667 173.333333,506.666667 C110.156241,526.507407 0.000000,608.000000 0.000000,608.000000 L -0.000000,768.000000 L 364.000000,768.000000 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Difference).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
	#[test]
	fn painted_dreams_5() {
		let a = path_from_path_data("M889.000000,0.000000C889.000000,0.000000 889.000000,21.000000 898.000000,46.000000 C909.595887,78.210796 872.365858,104.085306 869.000000,147.000000 C865.000000,198.000000 915.000000,237.000000 933.000000,273.000000 C951.000000,309.000000 951.703704,335.407407 923.000000,349.000000 C898.996281,360.366922 881.000000,367.000000 902.000000,394.000000 C923.000000,421.000000 928.592593,431.407407 898.000000,468.000000 C912.888889,472.888889 929.333333,513.333333 896.000000,523.000000 C896.000000,523.000000 876.000000,533.333333 886.000000,572.000000 C896.458810,612.440732 873.333333,657.777778 802.666667,656.444444 C738.670245,655.236965 689.000000,643.000000 655.000000,636.000000 C621.000000,629.000000 604.000000,623.000000 585.000000,666.000000 C566.000000,709.000000 564.000000,768.000000 564.000000,768.000000 C564.000000,768.000000 0.000000,768.000000 0.000000,768.000000 C0.000000,768.000000 0.000000,0.000000 0.000000,0.000000 C0.000000,0.000000 889.000000,0.000000 889.000000,0.000000 Z");
		let b = path_from_path_data(
			"M891.555556,569.382716C891.555556,569.382716 883.555556,577.777778 879.111111,595.851852 C874.666667,613.925926 857.185185,631.407407 830.814815,633.777778 C804.444444,636.148148 765.629630,637.925926 708.148148,616.296296 C650.666667,594.666667 560.666667,568.000000 468.000000,487.333333 C375.333333,406.666667 283.333333,354.666667 283.333333,354.666667 C332.000000,330.666667 373.407788,298.323579 468.479950,219.785706 C495.739209,197.267187 505.084065,165.580817 514.452332,146.721008 C525.711584,124.054345 577.519713,94.951389 589.958848,64.658436 C601.152263,37.399177 601.175694,0.000010 601.175694,0.000000 C601.175694,0.000000 0.000000,0.000000 0.000000,0.000000 C0.000000,0.000000 0.000000,768.000000 0.000000,768.000000 C0.000000,768.000000 891.555556,768.000000 891.555556,768.000000 C891.555556,768.000000 891.555556,569.382716 891.555556,569.382716 Z",
		);

		let result = path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, PathBooleanOperation::Intersection).unwrap();

		// Add assertions here based on expected results
		assert_eq!(result.len(), 1, "Expected 1 resulting path for Union operation");
		dbg!(path_to_path_data(&result[0], 0.001));
		// Add more specific assertions about the resulting path if needed
		assert!(!result[0].is_empty());
	}
}
