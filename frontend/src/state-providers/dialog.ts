import { writable } from "svelte/store";

import { type IconName } from "@graphite/utility-functions/icons";
import { type Editor } from "@graphite/wasm-communication/editor";
import { defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogButtons, UpdateDialogColumn1, UpdateDialogColumn2, patchWidgetLayout } from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: Editor) {
	const { subscribe, update } = writable({
		visible: false,
		title: "",
		icon: "" as IconName,
		buttons: defaultWidgetLayout(),
		column1: defaultWidgetLayout(),
		column2: defaultWidgetLayout(),
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor instance has panicked
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
	// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
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

	return {
		subscribe,
		dismissDialog,
		createCrashDialog,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
