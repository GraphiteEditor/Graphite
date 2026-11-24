export async function registerServiceWorker() {
	try {
		navigator.serviceWorker.addEventListener("controllerchange", () => {
			const controller = navigator.serviceWorker.controller;
			const reloadIfRemoved = async () => {
				if (controller?.state === "activated" && !(await caches.match(import.meta.url))) window.location.reload();
			};
			reloadIfRemoved();
			controller?.addEventListener("statechange", reloadIfRemoved);
		});

		const registration = await navigator.serviceWorker.register("/service-worker.js", { scope: "/" });

		const activeWorker = registration.active || registration.waiting || registration.installing;
		if (!activeWorker) return;

		const scheduleDeferredCaching = () => {
			if (activeWorker.state !== "activated") return;
			const sendMessage = () => registration.active?.postMessage({ type: "CACHE_DEFERRED" });
			if ("requestIdleCallback" in window) window.requestIdleCallback(sendMessage);
			else setTimeout(sendMessage, 5000); // Fallback to a delay for Safari which doesn't support `requestIdleCallback`
		};

		// Once the service worker is active, trigger deferred caching during idle time
		if (activeWorker.state === "activated") scheduleDeferredCaching();
		else activeWorker.addEventListener("statechange", scheduleDeferredCaching);
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error("Service worker registration failed:", err);
	}
}
