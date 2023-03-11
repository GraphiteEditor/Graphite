import { type Editor } from "~/src/wasm-communication/editor";
import { TriggerAboutGraphiteLocalizedCommitDate } from "~/src/wasm-communication/messages";

export function createLocalizationManager(editor: Editor): void {
	function localizeTimestamp(utc: string): string {
		// Timestamp
		const date = new Date(utc);
		if (Number.isNaN(date.getTime())) return utc;

		const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
			.formatToParts(new Date())
			.find((part) => part.type === "timeZoneName");

		const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
		const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
		const timezoneNameString = timezoneName?.value;
		return `${dateString} ${timeString} ${timezoneNameString}`;
	}

	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerAboutGraphiteLocalizedCommitDate, (triggerAboutGraphiteLocalizedCommitDate) => {
		const localized = localizeTimestamp(triggerAboutGraphiteLocalizedCommitDate.commitDate);
		editor.instance.requestAboutGraphiteDialogWithLocalizedCommitDate(localized);
	});
}
