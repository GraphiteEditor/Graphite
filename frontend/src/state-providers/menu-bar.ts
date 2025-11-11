import { writable } from "svelte/store";
import type { Editor } from "@graphite/editor";
import { defaultWidgetLayout, patchWidgetLayout, type WidgetLayout, type MenuListEntry, UpdateMenuBarLayout } from "@graphite/messages";

export function createMenuBarState(editor: Editor) {
    const state = writable({
        // Legacy entries used by the old menu bar renderer
        entries: [] as MenuListEntry[],
        // When true the frontend should render the WidgetLayout instead of `entries`
        useWidgetLayout: false,
        // The widget-backed layout (diffed)
        layout: defaultWidgetLayout() as WidgetLayout,
    });

    const { subscribe, update } = state;

    editor.subscriptions.subscribeJsMessage(UpdateMenuBarLayout, (updateMenuBarLayout) => {
        update((s) => {
            if ((updateMenuBarLayout as any).layout) {
                s.useWidgetLayout = false;
                s.entries = (updateMenuBarLayout as any).layout as MenuListEntry[];
                return s;
            }

            if ((updateMenuBarLayout as any).diff) {
                s.useWidgetLayout = true;
                patchWidgetLayout(s.layout, updateMenuBarLayout as any);
                // trigger reactivity by shallow copy
                s.layout = { ...s.layout } as WidgetLayout;
                return s;
            }

            return s;
        });
    });

    return { subscribe };
}

export type MenuBarState = ReturnType<typeof createMenuBarState>;
