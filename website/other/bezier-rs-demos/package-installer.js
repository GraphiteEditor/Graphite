// This script automatically installs the packages listed in yarn.lock and runs before `yarn start`.
// It skips the installation if this has already run and neither package.json nor yarn.lock has been modified since.

import { execSync } from "child_process";
import { existsSync, statSync, writeFileSync } from "fs";

const INSTALL_TIMESTAMP_FILE = ".install-timestamp";

// Checks if the install is needed by comparing modification times
const isInstallNeeded = () => {
	if (!existsSync(INSTALL_TIMESTAMP_FILE)) return true;

	const timestamp = statSync(INSTALL_TIMESTAMP_FILE).mtime;
	return ["package.json", "yarn.lock"].some((file) => {
		return existsSync(file) && statSync(file).mtime > timestamp;
	});
};

// Run `yarn install` if needed and update the install timestamp
if (isInstallNeeded()) {
	try {
		// eslint-disable-next-line no-console
		console.log("Installing yarn packages...");

		// Check if packages are up to date, doing so quickly by using `--immutable` and `--immutable-cache`.
		execSync("yarn install --immutable --immutable-cache", { stdio: "inherit" });

		// Touch the install timestamp file
		writeFileSync(INSTALL_TIMESTAMP_FILE, "");

		// eslint-disable-next-line no-console
		console.log("Finished installing yarn packages.");
	} catch (error) {
		// eslint-disable-next-line no-console
		console.error("Failed to install yarn packages. Please run `yarn install` from the `/website/other/bezier-rs-demos` directory.");
		process.exit(1);
	}
} else {
	// eslint-disable-next-line no-console
	console.log("All yarn packages are up-to-date.");
}
