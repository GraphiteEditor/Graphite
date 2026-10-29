//! How a person is shown: a stand-in name while unnamed, and a colour. Both derive from the user id alone, so every
//! peer shows the same ones without anything on the wire.

use document_graph_storage::UserId;

/// Spreads user ids, which may be small or sequential, over the whole range.
fn scramble(user: UserId) -> u64 {
	user.0.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

pub(crate) fn anonymous_name(user: UserId) -> String {
	let index = (scramble(user) >> 40) as usize % ANONYMOUS_ANIMALS.len();
	format!("Anonymous {}", ANONYMOUS_ANIMALS[index])
}

const ANONYMOUS_ANIMALS: [&str; 64] = [
	"Alligator",
	"Anteater",
	"Armadillo",
	"Axolotl",
	"Badger",
	"Bat",
	"Beaver",
	"Buffalo",
	"Camel",
	"Capybara",
	"Chameleon",
	"Cheetah",
	"Chinchilla",
	"Chipmunk",
	"Cormorant",
	"Coyote",
	"Crow",
	"Dingo",
	"Dolphin",
	"Duck",
	"Elephant",
	"Ferret",
	"Fox",
	"Frog",
	"Giraffe",
	"Gopher",
	"Grizzly",
	"Hedgehog",
	"Heron",
	"Hippo",
	"Hyena",
	"Ibex",
	"Iguana",
	"Jackal",
	"Jaguar",
	"Kangaroo",
	"Koala",
	"Kraken",
	"Lemur",
	"Leopard",
	"Liger",
	"Llama",
	"Manatee",
	"Mink",
	"Monkey",
	"Moose",
	"Narwhal",
	"Nyan Cat",
	"Orangutan",
	"Otter",
	"Panda",
	"Penguin",
	"Platypus",
	"Python",
	"Quagga",
	"Rabbit",
	"Raccoon",
	"Rhino",
	"Sheep",
	"Shrew",
	"Skunk",
	"Squirrel",
	"Tiger",
	"Turtle",
];

/// A hue picked by the user id, at a fixed saturation and lightness, as `#rrggbb`.
pub(crate) fn user_color(user: UserId) -> String {
	let hue = (scramble(user) >> 32) % 360;
	let (h, s, l): (f64, f64, f64) = (hue as f64 / 60., 0.65, 0.5);
	let c = (1. - (2. * l - 1.).abs()) * s;
	let x = c * (1. - (h % 2. - 1.).abs());
	let m = l - c / 2.;
	let (r, g, b) = match h as u32 {
		0 => (c, x, 0.),
		1 => (x, c, 0.),
		2 => (0., c, x),
		3 => (0., x, c),
		4 => (x, 0., c),
		_ => (c, 0., x),
	};
	let channel = |value: f64| ((value + m) * 255.).round() as u8;
	format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}
