import type { Editor } from "@graphite/editor";
import type { SubscriptionRouter } from "@graphite/subscription-router";
import { localizeTimestamp } from "@graphite/utility-functions/time";

let subscriptionsRef: SubscriptionRouter | undefined = undefined;
let editorRef: Editor | undefined = undefined;

export function createLocalizationManager(subscriptions: SubscriptionRouter, editor: Editor) {
	destroyLocalizationManager();

	subscriptionsRef = subscriptions;
	editorRef = editor;

	subscriptions.subscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate", (data) => {
		const localized = localizeTimestamp(data.commitDate);
		editor.requestAboutGraphiteDialogWithLocalizedCommitDate(localized.timestamp, localized.year);
	});
}

export function destroyLocalizationManager() {
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRef && editorRef) newModule?.createLocalizationManager(subscriptionsRef, editorRef);
});
