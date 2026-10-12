import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<number> = import.meta.hot?.data?.store || writable(0);
if (import.meta.hot) import.meta.hot.data.store = store;

// Hovering an entry may preview changes to other dropdowns, so they hold their widths until no dropdown's menu is open
export const openDropdownMenuCount = store;
