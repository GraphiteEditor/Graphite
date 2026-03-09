import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { Layout } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";
import type { IconName } from "@graphite/icons";
import { patchLayout } from "@graphite/utility-functions/widgets";

type DialogStoreState = {
	visible: boolean;
	title: string;
	icon: IconName | undefined;
	buttons: Layout;
	column1: Layout;
	column2: Layout;
	panicDetails: string;
};
const initialState: DialogStoreState = {
	visible: false,
	title: "",
	icon: undefined,
	buttons: [],
	column1: [],
	column2: [],
	// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor has panicked
	panicDetails: "",
};

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<DialogStoreState> = import.meta.hot?.data?.store || writable<DialogStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createDialogStore(editor: Editor) {
	// Subscribe to process backend events
	editor.subscriptions.subscribeFrontendMessage("DisplayDialog", (data) => {
		update((state) => {
			state.visible = true;

			state.title = data.title;
			state.icon = data.icon;

			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("DialogButtons", (data) => {
		update((state) => {
			patchLayout(state.buttons, data);

			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("DialogColumn1", (data) => {
		update((state) => {
			patchLayout(state.column1, data);

			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("DialogColumn2", (data) => {
		update((state) => {
			patchLayout(state.column2, data);

			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("DialogClose", () => {
		update((state) => {
			// Disallow dismissing the crash dialog since it should remain as the final notification
			if (state.panicDetails === "") state.visible = false;

			return state;
		});
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerDisplayThirdPartyLicensesDialog", async () => {
		const BACKUP_URL = "https://editor.graphite.art/third-party-licenses.txt";
		let licenseText = `Content was not able to load. Please check your network connection and try again.\n\nOr visit ${BACKUP_URL} for the license notices.`;

		const response = await fetch("/third-party-licenses.txt");
		if (response.ok && response.headers.get("Content-Type")?.includes("text/plain")) licenseText = await response.text();

		editor.handle.requestLicensesThirdPartyDialogWithLicenseText(licenseText);
	});

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("DisplayDialog");
		editor.subscriptions.unsubscribeFrontendMessage("DialogClose");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerDisplayThirdPartyLicensesDialog");
		editor.subscriptions.unsubscribeLayoutUpdate("DialogButtons");
		editor.subscriptions.unsubscribeLayoutUpdate("DialogColumn1");
		editor.subscriptions.unsubscribeLayoutUpdate("DialogColumn2");
	}

	currentCleanup = destroy;
	currentArgs = [editor];
	return {
		subscribe,
		destroy,
	};
}
export type DialogStore = ReturnType<typeof createDialogStore>;

// Creates a crash dialog from JS once the editor has panicked.
// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor has panicked so it cannot respond to widget callbacks.
export function createCrashDialog(panicDetails: string) {
	update((state) => {
		state.visible = true;

		state.icon = "Failure";
		state.title = "Crash";
		state.panicDetails = panicDetails;

		state.column1 = [];
		state.column2 = [];
		state.buttons = [];

		return state;
	});
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createDialogStore(...currentArgs);
});
