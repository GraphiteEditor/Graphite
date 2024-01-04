/* eslint-disable no-console */

import path from "path";

import { defineConfig } from "vite";

const projectRootDir = path.resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig({
	base: "",
	resolve: {
		alias: [{ find: "@", replacement: path.resolve(projectRootDir, "src") }],
	},
	server: {
		port: 8000,
		host: "0.0.0.0",
	},
});
