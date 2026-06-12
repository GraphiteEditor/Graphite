// This script automatically installs the npm packages listed in package-lock.json and runs as part of `npm run setup` (invoked by `cargo run`).
// It skips the installation if this has already run and neither package.json nor package-lock.json has been modified since.

import { execSync } from "child_process";
import { existsSync, statSync, writeFileSync } from "fs";

const INSTALL_TIMESTAMP_FILE = "node_modules/.install-timestamp";

// Checks if the install is needed by comparing modification times
const isInstallNeeded = () => {
	if (!existsSync(INSTALL_TIMESTAMP_FILE)) return true;

	const timestamp = statSync(INSTALL_TIMESTAMP_FILE).mtime;
	// This script is itself included so that changes to the install process below cause a reinstall
	return ["package.json", "package-lock.json", "package-installer.js"].some((file) => {
		return existsSync(file) && statSync(file).mtime > timestamp;
	});
};

// Run `npm ci` if needed and update the install timestamp
if (isInstallNeeded()) {
	try {
		// eslint-disable-next-line no-console
		console.log("Installing npm packages...");

		// Check if packages are up to date, doing so quickly by using `npm ci`, preferring local cached packages, and skipping the package audit and other checks.
		// The devDependencies are explicitly included because they hold the build tooling (Vite, etc.), which npm would
		// otherwise omit in environments that set NODE_ENV=production (like CI does for the sake of the Vite build).
		execSync("npm ci --include=dev --prefer-offline --no-audit --no-fund", { stdio: "inherit" });

		// Touch the install timestamp file
		writeFileSync(INSTALL_TIMESTAMP_FILE, "");

		// eslint-disable-next-line no-console
		console.log("Finished installing npm packages.");
	} catch (_) {
		// eslint-disable-next-line no-console
		console.error("\n\n--------------------> Failed to install npm packages. Please delete `/frontend/node_modules` then try again.\n\n");
		process.exit(1);
	}
} else {
	// eslint-disable-next-line no-console
	console.log("All npm packages are up-to-date.");
}
