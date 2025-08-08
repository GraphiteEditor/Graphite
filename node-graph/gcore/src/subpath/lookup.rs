// // use super::consts::DEFAULT_LUT_STEP_SIZE;
// use super::utils::{SubpathTValue, TValueType};
// use super::*;
// use glam::DVec2;

// /// Functionality relating to looking up properties of the `Subpath` or points along the `Subpath`.
// impl<PointId: super::structs::Identifier> Subpath<PointId> {
// 	/// Return a selection of equidistant points on the bezier curve.
// 	/// If no value is provided for `steps`, then the function will default `steps` to be 10.
// 	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/lookup-table/solo" title="Lookup-Table Demo"></iframe>
// 	pub fn compute_lookup_table(&self, steps: Option<usize>, tvalue_type: Option<TValueType>) -> Vec<DVec2> {
// 		let steps = steps.unwrap_or(DEFAULT_LUT_STEP_SIZE);
// 		let tvalue_type = tvalue_type.unwrap_or(TValueType::Parametric);

// 		(0..=steps)
// 			.map(|t| {
// 				let tvalue = match tvalue_type {
// 					TValueType::Parametric => SubpathTValue::GlobalParametric(t as f64 / steps as f64),
// 					TValueType::Euclidean => SubpathTValue::GlobalEuclidean(t as f64 / steps as f64),
// 				};
// 				self.evaluate(tvalue)
// 			})
// 			.collect()
// 	}
// }

// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
// 	use crate::utils::f64_compare;

// 	#[test]
// 	fn length_quadratic() {
// 		let start = DVec2::new(20., 30.);
// 		let middle = DVec2::new(80., 90.);
// 		let end = DVec2::new(60., 45.);
// 		let handle1 = DVec2::new(75., 85.);
// 		let handle2 = DVec2::new(40., 30.);
// 		let handle3 = DVec2::new(10., 10.);

// 		let bezier1 = Bezier::from_quadratic_dvec2(start, handle1, middle);
// 		let bezier2 = Bezier::from_quadratic_dvec2(middle, handle2, end);
// 		let bezier3 = Bezier::from_quadratic_dvec2(end, handle3, start);

// 		let mut subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: None,
// 					out_handle: Some(handle1),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: middle,
// 					in_handle: None,
// 					out_handle: Some(handle2),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: Some(handle3),
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);
// 		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None));

// 		subpath.closed = true;
// 		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None) + bezier3.length(None));
// 	}

// 	#[test]
// 	fn length_mixed() {
// 		let start = DVec2::new(20., 30.);
// 		let middle = DVec2::new(70., 70.);
// 		let end = DVec2::new(60., 45.);
// 		let handle1 = DVec2::new(75., 85.);
// 		let handle2 = DVec2::new(40., 30.);
// 		let handle3 = DVec2::new(10., 10.);

// 		let linear_bezier = Bezier::from_linear_dvec2(start, middle);
// 		let quadratic_bezier = Bezier::from_quadratic_dvec2(middle, handle1, end);
// 		let cubic_bezier = Bezier::from_cubic_dvec2(end, handle2, handle3, start);

// 		let mut subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: Some(handle3),
// 					out_handle: None,
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: middle,
// 					in_handle: None,
// 					out_handle: Some(handle1),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: Some(handle2),
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);
// 		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None));

// 		subpath.closed = true;
// 		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None) + cubic_bezier.length(None));
// 	}

// 	#[test]
// 	fn length_centroid() {
// 		let start = DVec2::new(0., 0.);
// 		let end = DVec2::new(1., 1.);
// 		let handle = DVec2::new(0., 1.);

// 		let mut subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: None,
// 					out_handle: Some(handle),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: None,
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);

// 		let expected_centroid = DVec2::new(0.4153039799983826, 0.5846960200016174);
// 		let epsilon = 0.00001;

// 		assert!(subpath.length_centroid_and_length(None, true).unwrap().0.abs_diff_eq(expected_centroid, epsilon));

// 		subpath.closed = true;
// 		assert!(subpath.length_centroid_and_length(None, true).unwrap().0.abs_diff_eq(expected_centroid, epsilon));
// 	}

// 	#[test]
// 	fn area() {
// 		let start = DVec2::new(0., 0.);
// 		let end = DVec2::new(1., 1.);
// 		let handle = DVec2::new(0., 1.);

// 		let mut subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: None,
// 					out_handle: Some(handle),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: None,
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);

// 		let expected_area = 1. / 3.;
// 		let epsilon = 0.00001;

// 		assert!((subpath.area(Some(0.001), Some(0.001)) - expected_area).abs() < epsilon);

// 		subpath.closed = true;
// 		assert!((subpath.area(Some(0.001), Some(0.001)) - expected_area).abs() < epsilon);
// 	}

// 	#[test]
// 	fn area_centroid() {
// 		let start = DVec2::new(0., 0.);
// 		let end = DVec2::new(1., 1.);
// 		let handle = DVec2::new(0., 1.);

// 		let mut subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: None,
// 					out_handle: Some(handle),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: None,
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);

// 		let expected_centroid = DVec2::new(0.4, 0.6);
// 		let epsilon = 0.00001;

// 		assert!(subpath.area_centroid(Some(0.001), Some(0.001), None).unwrap().abs_diff_eq(expected_centroid, epsilon));

// 		subpath.closed = true;
// 		assert!(subpath.area_centroid(Some(0.001), Some(0.001), None).unwrap().abs_diff_eq(expected_centroid, epsilon));
// 	}

// 	#[test]
// 	fn t_value_to_parametric_global_parametric_open_subpath() {
// 		let mock_manipulator_group = ManipulatorGroup {
// 			anchor: DVec2::new(0., 0.),
// 			in_handle: None,
// 			out_handle: None,
// 			id: EmptyId,
// 		};
// 		let open_subpath = Subpath {
// 			manipulator_groups: vec![mock_manipulator_group; 5],
// 			closed: false,
// 		};

// 		let (segment_index, t) = open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.7));
// 		assert_eq!(segment_index, 2);
// 		assert!(f64_compare(t, 0.8, MAX_ABSOLUTE_DIFFERENCE));

// 		// The start and end points of an open subpath are NOT equivalent
// 		assert_eq!(open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.)), (0, 0.));
// 		assert_eq!(open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(1.)), (3, 1.));
// 	}

// 	#[test]
// 	fn t_value_to_parametric_global_parametric_closed_subpath() {
// 		let mock_manipulator_group = ManipulatorGroup {
// 			anchor: DVec2::new(0., 0.),
// 			in_handle: None,
// 			out_handle: None,
// 			id: EmptyId,
// 		};
// 		let closed_subpath = Subpath {
// 			manipulator_groups: vec![mock_manipulator_group; 5],
// 			closed: true,
// 		};

// 		let (segment_index, t) = closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.7));
// 		assert_eq!(segment_index, 3);
// 		assert!(f64_compare(t, 0.5, MAX_ABSOLUTE_DIFFERENCE));

// 		// The start and end points of a closed subpath are equivalent
// 		assert_eq!(closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.)), (0, 0.));
// 		assert_eq!(closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(1.)), (4, 1.));
// 	}

// 	#[test]
// 	fn exact_start_end() {
// 		let start = DVec2::new(20., 30.);
// 		let end = DVec2::new(60., 45.);
// 		let handle = DVec2::new(75., 85.);

// 		let subpath: Subpath<EmptyId> = Subpath::from_bezier(&Bezier::from_quadratic_dvec2(start, handle, end));

// 		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(0.)), start);
// 		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(1.)), end);
// 	}
// }
