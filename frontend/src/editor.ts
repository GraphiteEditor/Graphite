// import { panicProxy } from "@graphite/utility-functions/panic-proxy";

import init, { EditorHandle, receiveNativeMessage } from "@graphite/../wasm/pkg/graphite_wasm";
import type { FrontendMessage } from "@graphite/../wasm/pkg/graphite_wasm";
import { createSubscriptionRouter } from "@graphite/subscription-router";
import type { MessageName, SubscriptionRouter } from "@graphite/subscription-router";
import { operatingSystem } from "@graphite/utility-functions/platform";

// Should be called asynchronously before `createEditor()`.
export async function initWasm() {
	// Import the Wasm module JS bindings and wrap them in the panic proxy
	const wasm = await init();
	for (const [name, f] of Object.entries(wasm)) {
		if (name.startsWith("__node_registry")) f();
	}

	window.imageCanvases = {};
	window.receiveNativeMessage = receiveNativeMessage;
}

// Should be called after running `initWasm()` and its promise resolving.
export function createEditor(): { editor: EditorHandle; subscriptions: SubscriptionRouter; destroy: () => void } {
	// Provide a random starter seed which must occur after initializing the Wasm module, since Wasm can't generate its own random numbers
	const randomSeedFloat = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
	const randomSeed = BigInt(randomSeedFloat);

	const editor = EditorHandle.create(operatingSystem(), randomSeed, (messageType: MessageName, messageData: FrontendMessage) => {
		// This callback is called by Wasm when a FrontendMessage is received from the editor backend
		subscriptions.handleFrontendMessage(messageType, messageData);
	});

	// Subscriptions: allows subscribing to messages in JS that are sent from the Wasm backend
	const subscriptions = createSubscriptionRouter();

	// Check if the URL hash fragment has any demo artwork to be loaded
	const demoArtworkAbortController = new AbortController();
	(async () => {
		const demoArtwork = window.location.hash.trim().match(/#demo\/(.*)/)?.[1];
		if (!demoArtwork) return;

		try {
			const url = new URL(`/demo-artwork/${demoArtwork}.${editor.fileExtension()}`, document.location.href);
			const data = await fetch(url, { signal: demoArtworkAbortController.signal });
			if (!data.ok) throw new Error();

			const filename = url.pathname.split("/").pop() || "Untitled";
			const content = await data.bytes();
			editor.openFile(`${filename}.${editor.fileExtension()}`, content);

			// Remove the hash fragment from the URL
			history.replaceState("", "", `${window.location.pathname}${window.location.search}`);
		} catch {
			// Do nothing
		}
	})();

	const destroy = () => {
		editor.free();
		demoArtworkAbortController.abort();
	};

	return { editor, subscriptions, destroy };
}

// Wasm state can't be hot-replaced, so we tell Vite to do a full page reload when this module changes
import.meta.hot?.accept(() => location.reload());
