import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { localizeTimestamp } from "/src/utility-functions/time";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;

export function createLocalizationManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyLocalizationManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;

	subscriptions.subscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate", (data) => {
		const localized = localizeTimestamp(data.commitDate);
		editor.requestAboutGraphiteDialogWithLocalizedCommitDate(localized.timestamp, localized.year);
	});
}

export function destroyLocalizationManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper) newModule?.createLocalizationManager(subscriptionsRouter, editorWrapper);
});
