use crate::RawImage;
use build_camera_data::build_camera_data;

pub struct CameraData {
	pub black: u16,
	pub maximum: u16,
	pub camera_to_xyz: [i16; 9],
}

impl CameraData {
	const DEFAULT: CameraData = CameraData {
		black: 0,
		maximum: 0,
		camera_to_xyz: [0; 9],
	};
}

const CAMERA_DATA: [(&str, CameraData); 40] = build_camera_data!();

const XYZ_TO_RGB: [[f64; 3]; 3] = [
	// Matrix:
	[0.412453, 0.357580, 0.180423],
	[0.212671, 0.715160, 0.072169],
	[0.019334, 0.119193, 0.950227],
];

impl RawImage {
	pub fn calculate_conversion_matrices(&mut self) {
		let Some(ref camera_model) = self.camera_model else { return };
		let camera_name_needle = camera_model.make.to_owned() + " " + &camera_model.model;

		let camera_to_xyz = CAMERA_DATA
			.iter()
			.find(|(camera_name_haystack, _)| camera_name_needle == *camera_name_haystack)
			.map(|(_, data)| data.camera_to_xyz.map(|x| (x as f64) / 10_000.));
		let Some(camera_to_xyz) = camera_to_xyz else { return };

		let mut camera_to_rgb = [[0.; 3]; 3];
		for i in 0..3 {
			for j in 0..3 {
				for k in 0..3 {
					camera_to_rgb[i][j] += camera_to_xyz[i * 3 + k] * XYZ_TO_RGB[k][j];
				}
			}
		}

		let white_balance_multiplier = camera_to_rgb.map(|x| 1. / x.iter().sum::<f64>());
		for (index, row) in camera_to_rgb.iter_mut().enumerate() {
			*row = row.map(|x| x * white_balance_multiplier[index]);
		}
		let rgb_to_camera = transpose(pseudoinverse(camera_to_rgb));

		let cfa_white_balance_multiplier = if let Some(white_balance) = self.camera_white_balance {
			white_balance
		} else {
			self.cfa_pattern.map(|index| white_balance_multiplier[index as usize])
		};

		self.white_balance = Some(cfa_white_balance_multiplier);
		self.camera_to_rgb = Some(camera_to_rgb);
		self.rgb_to_camera = Some(rgb_to_camera);
	}
}

#[allow(clippy::needless_range_loop)]
fn pseudoinverse<const N: usize>(matrix: [[f64; 3]; N]) -> [[f64; 3]; N] {
	let mut output_matrix = [[0.; 3]; N];
	let mut work = [[0.; 6]; 3];

	for i in 0..3 {
		for j in 0..6 {
			work[i][j] = if j == i + 3 { 1. } else { 0. };
		}
		for j in 0..3 {
			for k in 0..N {
				work[i][j] += matrix[k][i] * matrix[k][j];
			}
		}
	}

	for i in 0..3 {
		let num = work[i][i];
		for j in 0..6 {
			work[i][j] /= num;
		}
		for k in 0..3 {
			if k == i {
				continue;
			}
			let num = work[k][i];
			for j in 0..6 {
				work[k][j] -= work[i][j] * num;
			}
		}
	}

	for i in 0..N {
		for j in 0..3 {
			output_matrix[i][j] = 0.;
			for k in 0..3 {
				output_matrix[i][j] += work[j][k + 3] * matrix[i][k];
			}
		}
	}

	output_matrix
}

fn transpose<const N: usize>(matrix: [[f64; 3]; N]) -> [[f64; N]; 3] {
	let mut output_matrix = [[0.; N]; 3];

	for (i, row) in matrix.iter().enumerate() {
		for (j, &value) in row.iter().enumerate() {
			output_matrix[j][i] = value;
		}
	}

	output_matrix
}
