import { writable } from "svelte/store";

import type { Editor } from "@graphite/wasm-communication/editor";
import { defaultWidgetLayout, patchWidgetLayout, UpdatePropertyPanelOptionsLayout, UpdatePropertyPanelSectionsLayout } from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPropertiesState(editor: Editor) {
	const { subscribe, update } = writable({
		propertiesOptionsLayout: defaultWidgetLayout(),
		propertiesSectionsLayout: defaultWidgetLayout(),
	});

	editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelOptionsLayout, (updatePropertyPanelOptionsLayout) => {
		update((state) => {
			patchWidgetLayout(state.propertiesOptionsLayout, updatePropertyPanelOptionsLayout);
			return state;
		});
	});

	editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelSectionsLayout, (updatePropertyPanelSectionsLayout) => {
		update((state) => {
			patchWidgetLayout(state.propertiesSectionsLayout, updatePropertyPanelSectionsLayout);
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type PropertiesState = ReturnType<typeof createPropertiesState>;
