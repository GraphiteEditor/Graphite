import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import {
	defaultWidgetLayout,
	DisplayDialog,
	DisplayDialogDismiss,
	UpdateDialogButtons,
	UpdateDialogColumn1,
	UpdateDialogColumn2,
	patchWidgetLayout,
	TriggerDisplayThirdPartyLicensesDialog,
} from "@graphite/messages";
import { type IconName } from "@graphite/utility-functions/icons";

export function createDialogState(editor: Editor) {
	const { subscribe, update } = writable({
		visible: false,
		title: "",
		icon: "" as IconName,
		buttons: defaultWidgetLayout(),
		column1: defaultWidgetLayout(),
		column2: defaultWidgetLayout(),
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

			state.column1 = defaultWidgetLayout();
			state.column2 = defaultWidgetLayout();
			state.buttons = defaultWidgetLayout();

			return state;
		});
	}

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		update((state) => {
			state.visible = true;

			state.title = displayDialog.title;
			state.icon = displayDialog.icon;

			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDialogButtons, (updateDialogButtons) => {
		update((state) => {
			patchWidgetLayout(state.buttons, updateDialogButtons);

			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDialogColumn1, (updateDialogColumn1) => {
		update((state) => {
			patchWidgetLayout(state.column1, updateDialogColumn1);

			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDialogColumn2, (updateDialogColumn2) => {
		update((state) => {
			patchWidgetLayout(state.column2, updateDialogColumn2);

			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	editor.subscriptions.subscribeJsMessage(TriggerDisplayThirdPartyLicensesDialog, async () => {
		const BACKUP_URL = "https://editor.graphite.rs/third-party-licenses.txt";
		let licenseText = `Content was not able to load. Please check your network connection and try again.\n\nOr visit ${BACKUP_URL} for the license notices.`;
		if (editor.handle.inDevelopmentMode()) licenseText = `Third-party licenses are not available in development builds.\n\nVisit ${BACKUP_URL} for the license notices.`;

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
