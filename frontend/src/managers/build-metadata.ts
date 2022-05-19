import { Editor } from "@/interop/editor";

// Gets metadata populated in the `process.env` namespace by code in `frontend/vue.config.js`.
// TODO: Move that functionality to a build.rs file so our web build system is more lightweight.
export function createBuildMetadataManager(editor: Editor): void {
	// Release
	const release = process.env.VUE_APP_RELEASE_SERIES;

	// Timestamp
	const date = new Date(process.env.VUE_APP_COMMIT_DATE || "");
	const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
		.formatToParts(new Date())
		.find((part) => part.type === "timeZoneName");
	const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
	const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
	const timezoneNameString = timezoneName?.value;
	const timestamp = `${dateString} ${timeString} ${timezoneNameString}`;

	// Hash
	const hash = (process.env.VUE_APP_COMMIT_HASH || "").substring(0, 8);

	// Branch
	const branch = process.env.VUE_APP_COMMIT_BRANCH;

	editor.instance.populate_build_metadata(release || "", timestamp, hash, branch || "");
}
