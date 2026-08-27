import type { SubscriptionsRouter } from "/src/subscriptions-router";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

export function createHyperlinkManager(subscriptions: SubscriptionsRouter) {
	destroyHyperlinkManager();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("TriggerVisitLink", async (data) => {
		window.open(data.url, "_blank", "noopener");
	});
}

export function destroyHyperlinkManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerVisitLink");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter) newModule?.createHyperlinkManager(subscriptionsRouter);
});
