import { writable } from "svelte/store";

import type { Layout } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";
import type { IconName } from "@graphite/icons";
import { patchLayout } from "@graphite/utility-functions/widgets";

export function createDialogState(editor: Editor) {
	const { subscribe, update } = writable<{
		visible: boolean;
		title: string;
		icon: IconName | undefined;
		buttons: Layout;
		column1: Layout;
		column2: Layout;
		panicDetails: string;
	}>({
		visible: false,
		title: "",
		icon: undefined,
		buttons: [],
		column1: [],
		column2: [],
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor has panicked
		panicDetails: "",
	});

	function dismissDialog() {
		update((state) => {
			// Disallow dismissing the crash dialog since it can confuse users why the app stopped responding if they dismiss it without realizing what it means
			if (state.panicDetails === "") state.visible = false;

			return state;
		});
	}

	// Creates a crash dialog from JS once the editor has panicked.
	// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor has panicked so it cannot respond to widget callbacks.
	function createCrashDialog(panicDetails: string) {
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
	editor.subscriptions.subscribeFrontendMessage("DialogClose", dismissDialog);

	editor.subscriptions.subscribeFrontendMessage("TriggerDisplayThirdPartyLicensesDialog", async () => {
		const BACKUP_URL = "https://editor.graphite.art/third-party-licenses.txt";
		let licenseText = `Content was not able to load. Please check your network connection and try again.\n\nOr visit ${BACKUP_URL} for the license notices.`;

		const response = await fetch("/third-party-licenses.txt");
		if (response.ok && response.headers.get("Content-Type")?.includes("text/plain")) licenseText = await response.text();

		editor.handle.requestLicensesThirdPartyDialogWithLicenseText(licenseText);
	});

	return {
		subscribe,
		dismissDialog,
		createCrashDialog,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
