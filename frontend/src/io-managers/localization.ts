import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerAboutGraphiteLocalizedCommitDate } from "@graphite/wasm-communication/messages";

export function createLocalizationManager(editor: Editor) {
	function localizeTimestamp(utc: string): { timestamp: string; year: string } {
		// Timestamp
		const date = new Date(utc);
		if (Number.isNaN(date.getTime())) return { timestamp: utc, year: `${new Date().getFullYear()}` };

		const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
			.formatToParts(new Date())
			.find((part) => part.type === "timeZoneName");

		const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
		const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
		const timezoneNameString = timezoneName?.value;
		return { timestamp: `${dateString} ${timeString} ${timezoneNameString}`, year: String(date.getFullYear()) };
	}

	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerAboutGraphiteLocalizedCommitDate, (triggerAboutGraphiteLocalizedCommitDate) => {
		const localized = localizeTimestamp(triggerAboutGraphiteLocalizedCommitDate.commitDate);
		editor.instance.requestAboutGraphiteDialogWithLocalizedCommitDate(localized.timestamp, localized.year);
	});
}
