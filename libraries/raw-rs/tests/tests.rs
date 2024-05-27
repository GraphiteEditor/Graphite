use std::collections::HashMap;
use std::io::Cursor;

use libraw::Processor;

#[test_each::blob(glob = "libraries/raw-rs/tests/images/*", name(extension))]
fn test_raw_data(content: &[u8]) {
	let processor = Processor::new();
	let libraw_raw_image = processor.decode(content).unwrap();

	let mut content = Cursor::new(content);
	let raw_image = raw_rs::decode(&mut content).unwrap();

	if libraw_raw_image.sizes().raw_height as usize != raw_image.height {
		panic!("The height of raw image is {} but the expected value was {}", raw_image.height, libraw_raw_image.sizes().raw_height,);
	}

	if libraw_raw_image.sizes().raw_width as usize != raw_image.width {
		panic!("The width of raw image is {} but the expected value was {}", raw_image.width, libraw_raw_image.sizes().raw_width,);
	}

	if (*libraw_raw_image).len() != raw_image.data.len() {
		panic!("The size of data of raw image is {} but the expected value was {}", raw_image.data.len(), (*libraw_raw_image).len(),);
	}

	if (*libraw_raw_image) != raw_image.data {
		let mut histogram: HashMap<i32, usize> = HashMap::new();
		let mut non_zero_count: usize = 0;

		(*libraw_raw_image)
			.iter()
			.zip(raw_image.data.iter())
			.map(|(&a, &b)| {
				let a: i32 = a.into();
				let b: i32 = b.into();
				a - b
			})
			.filter(|&x| x != 0)
			.for_each(|x| {
				*histogram.entry(x).or_default() += 1;
				non_zero_count += 1;
			});

		let total_pixels = raw_image.height * raw_image.width;
		println!("{} ({:.5}%) pixels are different from expected", non_zero_count, non_zero_count as f64 / total_pixels as f64,);

		println!("Diff Histogram:");
		let mut items: Vec<_> = histogram.iter().map(|(&a, &b)| (a, b)).collect();
		items.sort();
		for (key, value) in items {
			println!("{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64)
		}

		panic!("The raw data does not match");
	}
}

// TODO: The glob below is purposefully kept empty to match nothing.
// Put the correct path after raw data to final image processing steps are implemented.
#[test_each::blob(glob = "", name(extension))]
fn test_final_image(content: &[u8]) {
	let processor = Processor::new();
	let libraw_image = processor.process_8bit(content).unwrap();

	let mut content = Cursor::new(content);
	let raw_image = raw_rs::decode(&mut content).unwrap();
	let image = raw_rs::process_8bit(raw_image);

	if libraw_image.height() as usize != image.height {
		panic!("The height of image is {} but the expected value was {}", image.height, libraw_image.height());
	}

	if libraw_image.width() as usize != image.width {
		panic!("The width of image is {} but the expected value was {}", image.width, libraw_image.width());
	}

	if (*libraw_image).len() != image.data.len() {
		panic!("The size of data of image is {} but the expected value was {}", image.data.len(), (*libraw_image).len(),);
	}

	if (*libraw_image) != image.data {
		let mut histogram_red: HashMap<i16, usize> = HashMap::new();
		let mut histogram_green: HashMap<i16, usize> = HashMap::new();
		let mut histogram_blue: HashMap<i16, usize> = HashMap::new();
		let mut non_zero_count: usize = 0;
		let mut non_zero_count_red: usize = 0;
		let mut non_zero_count_green: usize = 0;
		let mut non_zero_count_blue: usize = 0;

		(*libraw_image)
			.chunks_exact(3)
			.zip(image.data.chunks_exact(3))
			.map(|(a, b)| {
				let a: [u8; 3] = a.try_into().unwrap();
				let b: [u8; 3] = b.try_into().unwrap();
				(a, b)
			})
			.map(|([r1, g1, b1], [r2, g2, b2])| {
				let r1: i16 = r1.into();
				let g1: i16 = g1.into();
				let b1: i16 = b1.into();
				let r2: i16 = r2.into();
				let g2: i16 = g2.into();
				let b2: i16 = b2.into();
				[r1 - r2, g1 - g2, b1 - b2]
			})
			.filter(|&[r, g, b]| r != 0 || g != 0 || b != 0)
			.for_each(|[r, g, b]| {
				non_zero_count += 1;
				if r != 0 {
					*histogram_red.entry(r).or_default() += 1;
					non_zero_count_red += 1;
				}
				if g != 0 {
					*histogram_green.entry(g).or_default() += 1;
					non_zero_count_green += 1;
				}
				if b != 0 {
					*histogram_blue.entry(b).or_default() += 1;
					non_zero_count_blue += 1;
				}
			});

		let total_pixels = image.height * image.width;
		println!("{} ({:.5}%) pixels are different from expected", non_zero_count, non_zero_count as f64 / total_pixels as f64,);

		println!(
			"{} ({:.5}%) red pixels are different from expected",
			non_zero_count_red,
			non_zero_count_red as f64 / total_pixels as f64,
		);

		println!(
			"{} ({:.5}%) green pixels are different from expected",
			non_zero_count_green,
			non_zero_count_green as f64 / total_pixels as f64,
		);

		println!(
			"{} ({:.5}%) blue pixels are different from expected",
			non_zero_count_blue,
			non_zero_count_blue as f64 / total_pixels as f64,
		);

		println!("Diff Histogram for Red pixels:");
		let mut items: Vec<_> = histogram_red.iter().map(|(&a, &b)| (a, b)).collect();
		items.sort();
		for (key, value) in items {
			println!("{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64)
		}

		println!("Diff Histogram for Green pixels:");
		let mut items: Vec<_> = histogram_green.iter().map(|(&a, &b)| (a, b)).collect();
		items.sort();
		for (key, value) in items {
			println!("{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64)
		}

		println!("Diff Histogram for Blue pixels:");
		let mut items: Vec<_> = histogram_blue.iter().map(|(&a, &b)| (a, b)).collect();
		items.sort();
		for (key, value) in items {
			println!("{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64)
		}

		panic!("The final image does not match");
	}
}
