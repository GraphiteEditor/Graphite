// This script automatically installs the npm packages listed in package-lock.json and runs before `npm start`.
// It skips the installation if this has already run and neither package.json nor package-lock.json has been modified since.

import { execSync } from "child_process";
import { existsSync, statSync, writeFileSync } from "fs";

const INSTALL_TIMESTAMP_FILE = "node_modules/.install-timestamp";

// Checks if the install is needed by comparing modification times
const isInstallNeeded = () => {
	if (!existsSync(INSTALL_TIMESTAMP_FILE)) return true;

	const timestamp = statSync(INSTALL_TIMESTAMP_FILE).mtime;
	return ["package.json", "package-lock.json"].some((file) => {
		return existsSync(file) && statSync(file).mtime > timestamp;
	});
};

// Run `npm ci` if needed and update the install timestamp
if (isInstallNeeded()) {
	try {
		// eslint-disable-next-line no-console
		console.log("Installing npm packages...");

		// Check if packages are up to date, doing so quickly by using `npm ci`, preferring local cached packages, and skipping the package audit and other checks
		execSync("npm ci --prefer-offline --no-audit --no-fund", { stdio: "inherit" });

		// Touch the install timestamp file
		writeFileSync(INSTALL_TIMESTAMP_FILE, "");

		// eslint-disable-next-line no-console
		console.log("Finished installing npm packages.");
	} catch (_) {
		// eslint-disable-next-line no-console
		console.error("Failed to install npm packages. Please run `npm install` from the `/frontend` directory.");
		process.exit(1);
	}
} else {
	// eslint-disable-next-line no-console
	console.log("All npm packages are up-to-date.");
}
