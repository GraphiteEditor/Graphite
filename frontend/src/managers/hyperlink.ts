import type { SubscriptionRouter } from "@graphite/subscription-router";

let subscriptionsRef: SubscriptionRouter | undefined = undefined;

export function createHyperlinkManager(subscriptions: SubscriptionRouter) {
	destroyHyperlinkManager();

	subscriptionsRef = subscriptions;

	subscriptions.subscribeFrontendMessage("TriggerVisitLink", async (data) => {
		window.open(data.url, "_blank", "noopener");
	});
}

export function destroyHyperlinkManager() {
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerVisitLink");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRef) newModule?.createHyperlinkManager(subscriptionsRef);
});
