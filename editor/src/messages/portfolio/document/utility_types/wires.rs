use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use bezier_rs::{ManipulatorGroup, Subpath};
use glam::{DVec2, IVec2};
use graphene_std::uuid::NodeId;
use graphene_std::vector::PointId;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WirePath {
	#[serde(rename = "pathString")]
	pub path_string: String,
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub thick: bool,
	pub dashed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WirePathUpdate {
	pub id: NodeId,
	#[serde(rename = "inputIndex")]
	pub input_index: usize,
	// If none, then remove the wire from the map
	#[serde(rename = "wirePathUpdate")]
	pub wire_path_update: Option<WirePath>,
}

#[derive(Copy, Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum GraphWireStyle {
	#[default]
	Direct = 0,
	GridAligned = 1,
}

impl std::fmt::Display for GraphWireStyle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GraphWireStyle::GridAligned => write!(f, "Grid-Aligned"),
			GraphWireStyle::Direct => write!(f, "Direct"),
		}
	}
}

impl GraphWireStyle {
	pub fn tooltip_description(&self) -> &'static str {
		match self {
			GraphWireStyle::GridAligned => "Wires follow the grid, running in straight lines between nodes",
			GraphWireStyle::Direct => "Wires bend to run at an angle directly between nodes",
		}
	}

	pub fn is_direct(&self) -> bool {
		*self == GraphWireStyle::Direct
	}
}

pub fn build_vector_wire(output_position: DVec2, input_position: DVec2, vertical_out: bool, vertical_in: bool, graph_wire_style: GraphWireStyle) -> Subpath<PointId> {
	let grid_spacing = 24.;
	match graph_wire_style {
		GraphWireStyle::Direct => {
			let horizontal_gap = (output_position.x - input_position.x).abs();
			let vertical_gap = (output_position.y - input_position.y).abs();

			let curve_length = grid_spacing;
			let curve_falloff_rate = curve_length * std::f64::consts::TAU;

			let horizontal_curve_amount = -(2_f64.powf((-10. * horizontal_gap) / curve_falloff_rate)) + 1.;
			let vertical_curve_amount = -(2_f64.powf((-10. * vertical_gap) / curve_falloff_rate)) + 1.;
			let horizontal_curve = horizontal_curve_amount * curve_length;
			let vertical_curve = vertical_curve_amount * curve_length;

			let locations = [
				output_position,
				DVec2::new(
					if vertical_out { output_position.x } else { output_position.x + horizontal_curve },
					if vertical_out { output_position.y - vertical_curve } else { output_position.y },
				),
				DVec2::new(
					if vertical_in { input_position.x } else { input_position.x - horizontal_curve },
					if vertical_in { input_position.y + vertical_curve } else { input_position.y },
				),
				DVec2::new(input_position.x, input_position.y),
			];

			let smoothing = 0.5;
			let delta01 = DVec2::new((locations[1].x - locations[0].x) * smoothing, (locations[1].y - locations[0].y) * smoothing);
			let delta23 = DVec2::new((locations[3].x - locations[2].x) * smoothing, (locations[3].y - locations[2].y) * smoothing);

			Subpath::new(
				vec![
					ManipulatorGroup {
						anchor: locations[0],
						in_handle: None,
						out_handle: None,
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[1],
						in_handle: None,
						out_handle: Some(locations[1] + delta01),
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[2],
						in_handle: Some(locations[2] - delta23),
						out_handle: None,
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[3],
						in_handle: None,
						out_handle: None,
						id: PointId::generate(),
					},
				],
				false,
			)
		}
		GraphWireStyle::GridAligned => {
			let locations = straight_wire_paths(output_position, input_position, vertical_out, vertical_in);
			straight_wire_subpath(locations)
		}
	}
}

fn straight_wire_paths(output_position: DVec2, input_position: DVec2, vertical_out: bool, vertical_in: bool) -> Vec<IVec2> {
	let grid_spacing = 24;
	let line_width = 2;

	let in_x = input_position.x as i32;
	let in_y = input_position.y as i32;
	let out_x = output_position.x as i32;
	let out_y = output_position.y as i32;

	let mid_x = (in_x + out_x) / 2 + (((in_x + out_x) / 2) % grid_spacing);
	let mid_y = (in_y + out_y) / 2 + (((in_y + out_y) / 2) % grid_spacing);
	let mid_y_alternate = (in_y + in_y) / 2 - (((in_y + in_y) / 2) % grid_spacing);

	let x1 = out_x;
	let x2 = out_x + grid_spacing;
	let x3 = in_x - 2 * grid_spacing;
	let x4 = in_x;
	let x5 = in_x - 2 * grid_spacing + line_width;
	let x6 = out_x + grid_spacing + line_width;
	let x7 = out_x + 2 * grid_spacing + line_width;
	let x8 = in_x + line_width;
	let x9 = out_x + 2 * grid_spacing;
	let x10 = mid_x + line_width;
	let x11 = out_x - grid_spacing;
	let x12 = out_x - 4 * grid_spacing;
	let x13 = mid_x;
	let x14 = in_x + grid_spacing;
	let x15 = in_x - 4 * grid_spacing;
	let x16 = in_x + 8 * grid_spacing;
	let x17 = mid_x - 2 * line_width;
	let x18 = out_x + grid_spacing - 2 * line_width;
	let x19 = out_x - 2 * line_width;
	let x20 = mid_x - line_width;

	let y1 = out_y;
	let y2 = out_y - grid_spacing;
	let y3 = in_y;
	let y4 = out_y - grid_spacing + 5 * line_width + 1;
	let y5 = in_y - 2 * grid_spacing;
	let y6 = out_y + 4 * line_width;
	let y7 = out_y + 5 * line_width;
	let y8 = out_y - 2 * grid_spacing + 5 * line_width + 1;
	let y9 = out_y + 6 * line_width;
	let y10 = in_y + 2 * grid_spacing;
	let y111 = in_y + grid_spacing + 6 * line_width + 1;
	let y12 = in_y + grid_spacing - 5 * line_width + 1;
	let y13 = in_y - grid_spacing;
	let y14 = in_y + grid_spacing;
	let y15 = mid_y;
	let y16 = mid_y_alternate;

	let wire1 = vec![IVec2::new(x1, y1), IVec2::new(x1, y4), IVec2::new(x5, y4), IVec2::new(x5, y3), IVec2::new(x4, y3)];

	let wire2 = vec![IVec2::new(x1, y1), IVec2::new(x1, y16), IVec2::new(x3, y16), IVec2::new(x3, y3), IVec2::new(x4, y3)];

	let wire3 = vec![
		IVec2::new(x1, y1),
		IVec2::new(x1, y4),
		IVec2::new(x12, y4),
		IVec2::new(x12, y10),
		IVec2::new(x3, y10),
		IVec2::new(x3, y3),
		IVec2::new(x4, y3),
	];

	let wire4 = vec![
		IVec2::new(x1, y1),
		IVec2::new(x1, y4),
		IVec2::new(x13, y4),
		IVec2::new(x13, y10),
		IVec2::new(x3, y10),
		IVec2::new(x3, y3),
		IVec2::new(x4, y3),
	];

	if out_y == in_y && out_x > in_x && (vertical_out || !vertical_in) {
		return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y2), IVec2::new(x3, y2), IVec2::new(x3, y3), IVec2::new(x4, y3)];
	}

	// `outConnector` point and `inConnector` point lying on the same horizontal grid line and `outConnector` point lies to the right of `inConnector` point
	if out_y == in_y && out_x > in_x && (vertical_out || !vertical_in) {
		return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y2), IVec2::new(x3, y2), IVec2::new(x3, y3), IVec2::new(x4, y3)];
	};

	// Handle straight lines
	if out_y == in_y || (out_x == in_x && vertical_out) {
		return vec![IVec2::new(x1, y1), IVec2::new(x4, y3)];
	};

	// Handle standard right-angle paths
	// Start vertical, then horizontal

	// `outConnector` point lies to the left of `inConnector` point
	if vertical_out && in_x > out_x {
		// `outConnector` point lies above `inConnector` point
		if out_y < in_y {
			// `outConnector` point lies on the vertical grid line 4 units to the left of `inConnector` point point
			if -4 * grid_spacing <= out_x - in_x && out_x - in_x < -3 * grid_spacing {
				return wire1;
			};

			// `outConnector` point lying on vertical grid lines 3 and 2 units to the left of `inConnector` point
			if -3 * grid_spacing <= out_x - in_x && out_x - in_x <= -grid_spacing {
				if -2 * grid_spacing <= out_y - in_y && out_y - in_y <= -grid_spacing {
					return vec![IVec2::new(x1, y1), IVec2::new(x1, y2), IVec2::new(x2, y2), IVec2::new(x2, y3), IVec2::new(x4, y3)];
				};

				if -grid_spacing <= out_y - in_y && out_y - in_y <= 0 {
					return vec![IVec2::new(x1, y1), IVec2::new(x1, y4), IVec2::new(x6, y4), IVec2::new(x6, y3), IVec2::new(x4, y3)];
				};

				return vec![
					IVec2::new(x1, y1),
					IVec2::new(x1, y4),
					IVec2::new(x7, y4),
					IVec2::new(x7, y5),
					IVec2::new(x3, y5),
					IVec2::new(x3, y3),
					IVec2::new(x4, y3),
				];
			}

			// `outConnector` point lying on vertical grid line 1 units to the left of `inConnector` point
			if -grid_spacing < out_x - in_x && out_x - in_x <= 0 {
				// `outConnector` point lying on horizontal grid line 1 unit above `inConnector` point
				if -2 * grid_spacing <= out_y - in_y && out_y - in_y <= -grid_spacing {
					return vec![IVec2::new(x1, y6), IVec2::new(x2, y6), IVec2::new(x8, y3)];
				};

				// `outConnector` point lying on the same horizontal grid line as `inConnector` point
				if -grid_spacing <= out_y - in_y && out_y - in_y <= 0 {
					return vec![IVec2::new(x1, y7), IVec2::new(x4, y3)];
				};

				return vec![
					IVec2::new(x1, y1),
					IVec2::new(x1, y2),
					IVec2::new(x9, y2),
					IVec2::new(x9, y5),
					IVec2::new(x3, y5),
					IVec2::new(x3, y3),
					IVec2::new(x4, y3),
				];
			}

			return vec![IVec2::new(x1, y1), IVec2::new(x1, y4), IVec2::new(x10, y4), IVec2::new(x10, y3), IVec2::new(x4, y3)];
		}

		// `outConnector` point lies below `inConnector` point
		// `outConnector` point lying on vertical grid line 1 unit to the left of `inConnector` point
		if -grid_spacing <= out_x - in_x && out_x - in_x <= 0 {
			// `outConnector` point lying on the horizontal grid lines 1 and 2 units below the `inConnector` point
			if 0 <= out_y - in_y && out_y - in_y <= 2 * grid_spacing {
				return vec![IVec2::new(x1, y6), IVec2::new(x11, y6), IVec2::new(x11, y3), IVec2::new(x4, y3)];
			};

			return wire2;
		}

		return vec![IVec2::new(x1, y1), IVec2::new(x1, y3), IVec2::new(x4, y3)];
	}

	// `outConnector` point lies to the right of `inConnector` point
	if vertical_out && in_x <= out_x {
		// `outConnector` point lying on any horizontal grid line above `inConnector` point
		if out_y < in_y {
			// `outConnector` point lying on horizontal grid line 1 unit above `inConnector` point
			if -2 * grid_spacing < out_y - in_y && out_y - in_y <= -grid_spacing {
				return wire1;
			};

			// `outConnector` point lying on the same horizontal grid line as `inConnector` point
			if -grid_spacing < out_y - in_y && out_y - in_y <= 0 {
				return vec![IVec2::new(x1, y1), IVec2::new(x1, y8), IVec2::new(x5, y8), IVec2::new(x5, y3), IVec2::new(x4, y3)];
			};

			// `outConnector` point lying on vertical grid lines 1 and 2 units to the right of `inConnector` point
			if grid_spacing <= out_x - in_x && out_x - in_x <= 3 * grid_spacing {
				return vec![
					IVec2::new(x1, y1),
					IVec2::new(x1, y4),
					IVec2::new(x9, y4),
					IVec2::new(x9, y5),
					IVec2::new(x3, y5),
					IVec2::new(x3, y3),
					IVec2::new(x4, y3),
				];
			}

			return vec![
				IVec2::new(x1, y1),
				IVec2::new(x1, y4),
				IVec2::new(x10, y4),
				IVec2::new(x10, y5),
				IVec2::new(x5, y5),
				IVec2::new(x5, y3),
				IVec2::new(x4, y3),
			];
		}

		// `outConnector` point lies below `inConnector` point
		if out_y - in_y <= grid_spacing {
			// `outConnector` point lies on the horizontal grid line 1 unit below the `inConnector` Point
			if 0 <= out_x - in_x && out_x - in_x <= 13 * grid_spacing {
				return vec![IVec2::new(x1, y9), IVec2::new(x3, y9), IVec2::new(x3, y3), IVec2::new(x4, y3)];
			};

			if 13 < out_x - in_x && out_x - in_x <= 18 * grid_spacing {
				return wire3;
			};

			return wire4;
		}

		// `outConnector` point lies on the horizontal grid line 2 units below `outConnector` point
		if grid_spacing <= out_y - in_y && out_y - in_y <= 2 * grid_spacing {
			if 0 <= out_x - in_x && out_x - in_x <= 13 * grid_spacing {
				return vec![IVec2::new(x1, y7), IVec2::new(x5, y7), IVec2::new(x5, y3), IVec2::new(x4, y3)];
			};

			if 13 < out_x - in_x && out_x - in_x <= 18 * grid_spacing {
				return wire3;
			};

			return wire4;
		}

		// 0 to 4 units below the `outConnector` Point
		if out_y - in_y <= 4 * grid_spacing {
			return wire1;
		};

		return wire2;
	}

	// Start horizontal, then vertical
	if vertical_in {
		// when `outConnector` lies below `inConnector`
		if out_y > in_y {
			// `out_x` lies to the left of `in_x`
			if out_x < in_x {
				return vec![IVec2::new(x1, y1), IVec2::new(x4, y1), IVec2::new(x4, y3)];
			};

			// `out_x` lies to the right of `in_x`
			if out_y - in_y <= grid_spacing {
				// `outConnector` point directly below `inConnector` point
				if 0 <= out_x - in_x && out_x - in_x <= grid_spacing {
					return vec![IVec2::new(x1, y1), IVec2::new(x14, y1), IVec2::new(x14, y2), IVec2::new(x4, y2), IVec2::new(x4, y3)];
				};

				// `outConnector` point lies below `inConnector` point and strictly to the right of `inConnector` point
				return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y111), IVec2::new(x4, y111), IVec2::new(x4, y3)];
			}

			return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y2), IVec2::new(x4, y2), IVec2::new(x4, y3)];
		}

		// `out_y` lies on or above the `in_y` point
		if -6 * grid_spacing < in_x - out_x && in_x - out_x < 4 * grid_spacing {
			// edge case: `outConnector` point lying on vertical grid lines ranging from 4 units to left to 5 units to right of `inConnector` point
			if -grid_spacing < in_x - out_x && in_x - out_x < 4 * grid_spacing {
				return vec![
					IVec2::new(x1, y1),
					IVec2::new(x2, y1),
					IVec2::new(x2, y2),
					IVec2::new(x15, y2),
					IVec2::new(x15, y12),
					IVec2::new(x4, y12),
					IVec2::new(x4, y3),
				];
			}

			return vec![IVec2::new(x1, y1), IVec2::new(x16, y1), IVec2::new(x16, y12), IVec2::new(x4, y12), IVec2::new(x4, y3)];
		}

		// left of edge case: `outConnector` point lying on vertical grid lines more than 4 units to left of `inConnector` point
		if 4 * grid_spacing < in_x - out_x {
			return vec![IVec2::new(x1, y1), IVec2::new(x17, y1), IVec2::new(x17, y12), IVec2::new(x4, y12), IVec2::new(x4, y3)];
		};

		// right of edge case: `outConnector` point lying on the vertical grid lines more than 5 units to right of `inConnector` point
		if 6 * grid_spacing > in_x - out_x {
			return vec![IVec2::new(x1, y1), IVec2::new(x18, y1), IVec2::new(x18, y12), IVec2::new(x4, y12), IVec2::new(x4, y3)];
		};
	}

	// Both horizontal - use horizontal middle point
	// When `inConnector` point is one of the two closest diagonally opposite points
	if 0 <= in_x - out_x && in_x - out_x <= grid_spacing && in_y - out_y >= -grid_spacing && in_y - out_y <= grid_spacing {
		return vec![IVec2::new(x19, y1), IVec2::new(x19, y3), IVec2::new(x4, y3)];
	}

	// When `inConnector` point lies on the horizontal line 1 unit above and below the `outConnector` point
	if -grid_spacing <= out_y - in_y && out_y - in_y <= grid_spacing && out_x > in_x {
		// Horizontal line above `out_y`
		if in_y < out_y {
			return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y13), IVec2::new(x3, y13), IVec2::new(x3, y3), IVec2::new(x4, y3)];
		};

		// Horizontal line below `out_y`
		return vec![IVec2::new(x1, y1), IVec2::new(x2, y1), IVec2::new(x2, y14), IVec2::new(x3, y14), IVec2::new(x3, y3), IVec2::new(x4, y3)];
	}

	// `outConnector` point to the right of `inConnector` point
	if out_x > in_x - grid_spacing {
		return vec![
			IVec2::new(x1, y1),
			IVec2::new(x18, y1),
			IVec2::new(x18, y15),
			IVec2::new(x5, y15),
			IVec2::new(x5, y3),
			IVec2::new(x4, y3),
		];
	};

	// When `inConnector` point lies on the vertical grid line two units to the right of `outConnector` point
	if grid_spacing <= in_x - out_x && in_x - out_x <= 2 * grid_spacing {
		return vec![IVec2::new(x1, y1), IVec2::new(x18, y1), IVec2::new(x18, y3), IVec2::new(x4, y3)];
	};

	vec![IVec2::new(x1, y1), IVec2::new(x20, y1), IVec2::new(x20, y3), IVec2::new(x4, y3)]
}

fn straight_wire_subpath(locations: Vec<IVec2>) -> Subpath<PointId> {
	if locations.is_empty() {
		return Subpath::new(Vec::new(), false);
	}

	if locations.len() == 2 {
		return Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: locations[0].into(),
					in_handle: None,
					out_handle: None,
					id: PointId::generate(),
				},
				ManipulatorGroup {
					anchor: locations[1].into(),
					in_handle: None,
					out_handle: None,
					id: PointId::generate(),
				},
			],
			false,
		);
	}

	let corner_radius = 10;

	// Create path with rounded corners
	let mut path = vec![ManipulatorGroup {
		anchor: locations[0].into(),
		in_handle: None,
		out_handle: None,
		id: PointId::generate(),
	}];

	for i in 1..(locations.len() - 1) {
		let prev = locations[i - 1];
		let curr = locations[i];
		let next = locations[i + 1];

		let corner_start = IVec2::new(
			curr.x
				+ if curr.x == prev.x {
					0
				} else if prev.x > curr.x {
					corner_radius
				} else {
					-corner_radius
				},
			curr.y
				+ if curr.y == prev.y {
					0
				} else if prev.y > curr.y {
					corner_radius
				} else {
					-corner_radius
				},
		);

		let corner_start_mid = IVec2::new(
			curr.x
				+ if curr.x == prev.x {
					0
				} else if prev.x > curr.x {
					corner_radius / 2
				} else {
					-corner_radius / 2
				},
			curr.y
				+ if curr.y == prev.y {
					0
				} else {
					match prev.y > curr.y {
						true => corner_radius / 2,
						false => -corner_radius / 2,
					}
				},
		);

		let corner_end = IVec2::new(
			curr.x
				+ if curr.x == next.x {
					0
				} else if next.x > curr.x {
					corner_radius
				} else {
					-corner_radius
				},
			curr.y
				+ if curr.y == next.y {
					0
				} else if next.y > curr.y {
					corner_radius
				} else {
					-corner_radius
				},
		);

		let corner_end_mid = IVec2::new(
			curr.x
				+ if curr.x == next.x {
					0
				} else if next.x > curr.x {
					corner_radius / 2
				} else {
					-corner_radius / 2
				},
			curr.y
				+ if curr.y == next.y {
					0
				} else if next.y > curr.y {
					10 / 2
				} else {
					-corner_radius / 2
				},
		);

		path.extend(vec![
			ManipulatorGroup {
				anchor: corner_start.into(),
				in_handle: None,
				out_handle: Some(corner_start_mid.into()),
				id: PointId::generate(),
			},
			ManipulatorGroup {
				anchor: corner_end.into(),
				in_handle: Some(corner_end_mid.into()),
				out_handle: None,
				id: PointId::generate(),
			},
		])
	}

	path.push(ManipulatorGroup {
		anchor: (*locations.last().unwrap()).into(),
		in_handle: None,
		out_handle: None,
		id: PointId::generate(),
	});
	Subpath::new(path, false)
}
