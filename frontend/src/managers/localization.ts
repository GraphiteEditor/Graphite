import type { Editor } from "@graphite/editor";

let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;

export function createLocalizationManager(editor: Editor): () => void {
	currentArgs = [editor];
	// Subscribe to process backend event
	editor.subscriptions.subscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate", (data) => {
		const localized = localizeTimestamp(data.commitDate);
		editor.handle.requestAboutGraphiteDialogWithLocalizedCommitDate(localized.timestamp, localized.year);
	});

	currentCleanup = () => {
		editor.subscriptions.unsubscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate");
	};
	return currentCleanup;
}

function localizeTimestamp(utc: string): { timestamp: string; year: string } {
	// Timestamp
	const date = new Date(utc);
	if (Number.isNaN(date.getTime())) return { timestamp: utc, year: `${new Date().getFullYear()}` };

	const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "longGeneric" })
		.formatToParts(new Date())
		.find((part) => part.type === "timeZoneName");

	const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
	const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
	const timezoneNameString = timezoneName?.value;
	return { timestamp: `${dateString} ${timeString} ${timezoneNameString}`, year: String(date.getFullYear()) };
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createLocalizationManager(...currentArgs);
});
