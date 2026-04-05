export async function registerServiceWorker() {
	try {
		const registration = await navigator.serviceWorker.register("/service-worker.js", { scope: "/" });

		// When a new service worker is found, auto-reload once it activates
		registration.addEventListener("updatefound", () => {
			const newWorker = registration.installing;
			newWorker?.addEventListener("statechange", () => {
				// Only reload if there was a previous controller, meaning this is an update, not first install
				if (newWorker.state === "activated" && navigator.serviceWorker.controller) window.location.reload();
			});
		});

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
