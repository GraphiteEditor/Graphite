import { createCrashDialog } from "@graphite/stores/dialog";
import type { SubscriptionRouter } from "@graphite/subscription-router";

let subscriptionsRef: SubscriptionRouter | undefined = undefined;

export function createPanicManager(subscriptions: SubscriptionRouter) {
	destroyPanicManager();

	subscriptionsRef = subscriptions;

	subscriptions.subscribeFrontendMessage("DisplayDialogPanic", (data) => {
		// `Error.stackTraceLimit` is only available in V8/Chromium
		const previousStackTraceLimit = Error.stackTraceLimit;
		Error.stackTraceLimit = Infinity;
		const stackTrace = new Error().stack || "";
		Error.stackTraceLimit = previousStackTraceLimit;

		const panicDetails = `${data.panicInfo}${stackTrace ? `\n\n${stackTrace}` : ""}`;

		// eslint-disable-next-line no-console
		console.error(panicDetails);

		createCrashDialog(panicDetails);
	});
}

export function destroyPanicManager() {
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("DisplayDialogPanic");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRef) newModule?.createPanicManager(subscriptionsRef);
});
