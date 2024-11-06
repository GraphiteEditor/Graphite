use crate::RawImage;
use rawkit_proc_macros::build_camera_data;

pub struct CameraData {
	pub black: u16,
	pub maximum: u16,
	pub xyz_to_camera: [i16; 9],
}

impl CameraData {
	const DEFAULT: CameraData = CameraData {
		black: 0,
		maximum: 0,
		xyz_to_camera: [0; 9],
	};
}

const CAMERA_DATA: [(&str, CameraData); 40] = build_camera_data!();

const RGB_TO_XYZ: [[f64; 3]; 3] = [
	// Matrix:
	[0.412453, 0.357580, 0.180423],
	[0.212671, 0.715160, 0.072169],
	[0.019334, 0.119193, 0.950227],
];

impl RawImage {
	pub fn calculate_conversion_matrices(&mut self) {
		let Some(ref camera_model) = self.camera_model else { return };
		let camera_name_needle = camera_model.make.to_owned() + " " + &camera_model.model;

		let xyz_to_camera = CAMERA_DATA
			.iter()
			.find(|(camera_name_haystack, _)| camera_name_needle == *camera_name_haystack)
			.map(|(_, data)| data.xyz_to_camera.map(|x| (x as f64) / 10_000.));
		let Some(xyz_to_camera) = xyz_to_camera else { return };

		let mut rgb_to_camera = [[0.; 3]; 3];
		for i in 0..3 {
			for j in 0..3 {
				for k in 0..3 {
					rgb_to_camera[i][j] += RGB_TO_XYZ[k][j] * xyz_to_camera[i * 3 + k];
				}
			}
		}

		let white_balance_multiplier = rgb_to_camera.map(|x| 1. / x.iter().sum::<f64>());
		for (index, row) in rgb_to_camera.iter_mut().enumerate() {
			*row = row.map(|x| x * white_balance_multiplier[index]);
		}
		let camera_to_rgb = transpose(pseudoinverse(rgb_to_camera));

		let cfa_white_balance_multiplier = if let Some(white_balance) = self.camera_white_balance {
			white_balance
		} else {
			self.cfa_pattern.map(|index| white_balance_multiplier[index as usize])
		};

		self.white_balance = Some(cfa_white_balance_multiplier);
		self.camera_to_rgb = Some(camera_to_rgb);
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
