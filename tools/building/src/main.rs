use building::*;

fn usage() {
	eprintln!("usage: cargo run [<command>] [release|debug|profiling]");
	eprintln!();
	eprintln!("commands:");
	eprintln!("  web               Run the dev server");
	eprintln!("  web build         Build the web version");
	eprintln!("  desktop           Run the desktop app");
	eprintln!("  desktop build     Build the desktop version");
	eprintln!("  check             Check that all required dependencies are installed");
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let args: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();

	match args.as_slice() {
		["desktop", rest @ ..] => match rest {
			["build", rest @ ..] => build_desktop(rest.into()),
			_ => run_desktop(rest.into()),
		},
		["web", rest @ ..] => match rest {
			["build", rest @ ..] => build_web(rest.into()),
			_ => run_web(rest.into()),
		},
		rest => match rest {
			["build", rest @ ..] => build_web(rest.into()),
			_ => run_web(rest.into()),
		},
	}
}

fn run_web(profile: Profile) {
	match profile {
		Profile::Debug | Profile::Default => run_in_frontend_dir("npm run start"),
		Profile::Release => run_in_frontend_dir("npm run production"),
		Profile::Profiling => run_in_frontend_dir("npm run profiling"),
		Profile::Error => usage(),
	}
}

fn run_desktop(profile: Profile) {
	match profile {
		Profile::Debug | Profile::Default => {
			run_in_frontend_dir("npm run build-native-dev");
			run("cargo run -p third-party-licenses --features desktop");
			run("cargo run -p graphite-desktop-bundle -- open");
		}
		Profile::Release => {
			run_in_frontend_dir("npm run build-native");
			run("cargo run -p third-party-licenses --features desktop");
			run("cargo run -r -p graphite-desktop-bundle -- open");
		}
		Profile::Profiling => todo!("profiling run for desktop"),
		Profile::Error => usage(),
	}
}

fn build_web(profile: Profile) {
	match profile {
		Profile::Debug => run_in_frontend_dir("npm run build-dev"),
		Profile::Release | Profile::Default => run_in_frontend_dir("npm run build"),
		Profile::Profiling => run_in_frontend_dir("npm run build-profiling"),
		Profile::Error => usage(),
	}
}

fn build_desktop(profile: Profile) {
	match profile {
		Profile::Debug => {
			run_in_frontend_dir("npm run build-native-dev");
			run("cargo run -p third-party-licenses --features desktop");
			run("cargo run -p graphite-desktop-bundle");
		}
		Profile::Release | Profile::Default => {
			run_in_frontend_dir("npm run build-native");
			run("cargo run -p third-party-licenses --features desktop");
			run("cargo run -r -p graphite-desktop-bundle");
		}
		Profile::Profiling => todo!("profiling build for desktop"),
		Profile::Error => usage(),
	}
}
