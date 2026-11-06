import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<number> = import.meta.hot?.data?.store || writable(0);
if (import.meta.hot) import.meta.hot.data.store = store;
const openedByArrowKeysStore: Writable<boolean> = import.meta.hot?.data?.openedByArrowKeysStore || writable(false);
if (import.meta.hot) import.meta.hot.data.openedByArrowKeysStore = openedByArrowKeysStore;
const keyboardLeadsStore: Writable<boolean> = import.meta.hot?.data?.keyboardLeadsStore || writable(false);
if (import.meta.hot) import.meta.hot.data.keyboardLeadsStore = keyboardLeadsStore;

// Hovering an entry may preview changes to other dropdowns, so they hold their widths until no dropdown's menu is open
export const openDropdownMenuCount = store;

// Set while the arrow keys move from one menu to its neighbor, like between the menu bar's, so the menu opening starts on its first entry
export const menuOpenedByArrowKeys = openedByArrowKeysStore;

// Set while the arrow keys, rather than the pointer, last moved through the open menu bar menu, so its titles don't draw the pointer's hover
export const menuBarKeyboardLeads = keyboardLeadsStore;
