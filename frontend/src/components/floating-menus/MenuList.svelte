<svelte:options accessors={true} />

<script lang="ts">
	import { createEventDispatcher, tick, onDestroy } from "svelte";
	import { get } from "svelte/store";
	import MenuList from "/src/components/floating-menus/MenuList.svelte";
	import FloatingMenu, { preventEscapeClosingParentFloatingMenu } from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import TextInput from "/src/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import ShortcutLabel from "/src/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import { menuBarKeyboardLeads, menuOpenedByArrowKeys } from "/src/stores/dropdown-menus";
	import { targetIsTextField } from "/src/utility-functions/input";
	import type { MenuListEntry, MenuDirection } from "/wrapper/pkg/graphite_wasm_wrapper";

	// What a menu tells the menu it's nested in about a key that needs more than this menu to handle
	type KeyOutcome = "Handled" | "Unhandled" | "CloseSubmenu" | "NextMenu" | "PreviousMenu";

	const ENTRY_HEIGHT = 20;

	let self: FloatingMenu | undefined;
	let scroller: LayoutCol | undefined;
	let searchTextInput: TextInput | undefined;
	// Only the open submenu exists, so it's the only one referenced
	let openChild: MenuList | undefined;

	const dispatch = createEventDispatcher<{
		open: boolean;
		activeEntry: MenuListEntry;
		selectedEntryValuePath: string[];
		preview: MenuListEntry;
		naturalWidth: number;
	}>();

	export let parentsValuePath: string[] = [];
	export let entries: MenuListEntry[][];
	export let entriesHash: bigint;
	export let activeEntry: MenuListEntry | undefined = undefined;
	export let open: boolean;
	export let direction: MenuDirection = "Bottom";
	export let minWidth = 0;
	export let drawIcon = false;
	export let monospace = false;
	export let scrollableY = false;
	export let virtualScrolling = false;
	// Whether to measure the menu's width for its spawner to size itself by
	export let measuresNaturalWidth = false;
	// Whether the highlight moves to each new active entry given while the menu is open
	export let highlightFollowsActiveEntry = false;
	// Whether the menu holds a value, as a dropdown does, which its highlight shows and the arrow keys change
	export let holdsValue = false;
	// Whether ← and → in the top level open the neighboring menus that hovering also moves between, as in the menu bar
	export let arrowKeysOpenNeighborMenus = false;
	// Set by the menu this one is nested in
	export let submenu = false;

	let openChildValue: string | undefined = undefined;
	let search = "";
	let reactiveEntries = entries;
	// The entry chosen by the arrow keys, or the value of a menu holding one, drawn in white
	let highlighted: MenuListEntry | undefined = activeEntry;
	// The entry under the pointer, drawn in gray
	let hovered: MenuListEntry | undefined = undefined;
	// Whether the pointer moved onto an entry since the arrow keys last moved the highlight, so the keys pick up from where it points
	let pointerLed = false;
	// The value a menu holding one opened with, which the arrow keys may change before it closes
	let valueWhenOpened: MenuListEntry | undefined = undefined;
	let lastPreviewedValue: string | undefined = undefined;
	let lastActiveEntry: { value: string | undefined; entriesHash: bigint } = { value: activeEntry?.value, entriesHash };
	let wasOpen = false;
	let lastPointerPosition: { x: number; y: number } | undefined = undefined;
	// Whether the wheel scrolled the menu since the last key press, so the entries it scrolls under the resting pointer take the hover
	let wheelScrolled = false;
	let virtualScrollingEntriesStart = 0;
	let maxMenuWidth = 0;
	let resizeObserver: ResizeObserver | undefined = undefined;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- `loadedFonts` reactivity is driven by `loadedFontsGeneration`, not the Set itself
	let loadedFonts = new Set<string>();
	let loadedFontsGeneration = 0;

	$: watchOpen(open);
	$: watchEntriesHash(entriesHash);
	$: watchRemeasureWidth(reactiveEntries, drawIcon, self, measuresNaturalWidth, search);
	$: watchFilteredEntries(filteredEntries, open);
	$: watchActiveEntry(activeEntry, entriesHash);
	$: watchPreviewedEntry(holdsValue && open ? (pointerLed && hovered) || highlighted : undefined);

	$: virtualScrollingEntryHeight = virtualScrolling ? ENTRY_HEIGHT : 0;
	$: filteredEntries = reactiveEntries.map((section) => section.filter((entry) => inSearch(search, entry)));
	$: startIndex = virtualScrollingEntryHeight ? virtualScrollingStartIndex : 0;
	// Virtual scrolling calculations
	$: virtualScrollingTotalHeight = filteredEntries.length === 0 ? 0 : filteredEntries[0].length * virtualScrollingEntryHeight;
	$: virtualScrollingStartIndex = filteredEntries.length === 0 ? 0 : Math.floor(virtualScrollingEntriesStart / virtualScrollingEntryHeight) || 0;
	$: virtualScrollingEndIndex = filteredEntries.length === 0 ? 0 : Math.min(filteredEntries[0].length, virtualScrollingStartIndex + 1 + 400 / virtualScrollingEntryHeight);

	onDestroy(() => {
		listenWhileOpen(false);
		resizeObserver?.disconnect();
	});

	function watchOpen(open: boolean) {
		if (open === wasOpen) return;
		wasOpen = open;

		// Closing a menu holding a value keeps whichever value the arrow keys moved it to, unless Escape put back the one it opened with
		if (!open && holdsValue && highlighted && highlighted.value !== valueWhenOpened?.value) dispatch("activeEntry", highlighted);

		// The top-level menu handles the keys and the pointer for its submenus
		if (!submenu) listenWhileOpen(open);

		// For virtual scrolling menus, observe width changes so the menu only grows and never shrinks while open
		if (open && virtualScrolling) {
			startMenuWidthObserver();
		} else if (resizeObserver) {
			resizeObserver.disconnect();
			resizeObserver = undefined;
			maxMenuWidth = 0;
		}

		highlighted = activeEntry;
		hovered = undefined;
		pointerLed = false;
		valueWhenOpened = activeEntry;
		lastPreviewedValue = activeEntry?.value;
		openChildValue = undefined;
		search = "";

		if (arrowKeysOpenNeighborMenus) menuBarKeyboardLeads.set(false);

		// Opened by the arrow keys from a neighboring menu, it starts on its first entry like the menu it took over from
		if (open && !submenu && get(menuOpenedByArrowKeys)) {
			menuOpenedByArrowKeys.set(false);
			highlighted = selectableEntries()[0];
		}

		dispatch("open", open);
	}

	function listenWhileOpen(listening: boolean) {
		// TODO: Move keyboard input handling entirely to the unified system in `input.ts`.
		// TODO: The current approach is hacky and blocks the allowances for shortcuts like the key to open the browser's dev tools.
		if (listening) {
			addEventListener("keydown", keydown);
			addEventListener("pointermove", onPointerMove);
			addEventListener("wheel", onWheel, { capture: true, passive: true });
			addEventListener("scroll", onScrollWhileOpen, true);
		} else {
			removeEventListener("keydown", keydown);
			removeEventListener("pointermove", onPointerMove);
			removeEventListener("wheel", onWheel, { capture: true });
			removeEventListener("scroll", onScrollWhileOpen, true);
		}
	}

	// Keeps the highlight on an entry the search still shows, or gives it to the first result for Enter to pick, and keeps it in view
	async function watchFilteredEntries(filteredEntries: MenuListEntry[][], open: boolean) {
		if (!open) return;

		const shown = filteredEntries.flat();
		const isShown = (entry: MenuListEntry | undefined) => entry !== undefined && shown.some((candidate) => candidate.value === entry.value);

		if (search && !isShown(highlighted)) highlighted = shown.find((entry) => !entry.disabled);
		if (hovered && !isShown(hovered)) hovered = undefined;
		if (openChildValue !== undefined && !shown.some((entry) => entry.value === openChildValue)) openChildValue = undefined;

		// Allows the scrollable area to expand if necessary
		await tick();

		if (highlighted) setHighlighted(highlighted);
	}

	// An active entry given while the menu is open takes the highlight and is scrolled into view, unless it's the same one among the same
	// entries given again, as any layout update does, which leaves the highlight wherever it was moved
	function watchActiveEntry(activeEntry: MenuListEntry | undefined, entriesHash: bigint) {
		const given = { value: activeEntry?.value, entriesHash };
		const repeated = given.value === lastActiveEntry.value && given.entriesHash === lastActiveEntry.entriesHash;
		lastActiveEntry = given;
		if (!highlightFollowsActiveEntry || !open || !activeEntry || repeated) return;

		highlighted = activeEntry;
		pointerLed = false;
		// Once its row is drawn, unless the highlight has moved on by then
		tick().then(() => {
			if (open && highlighted?.value === activeEntry.value) setHighlighted(activeEntry);
		});
	}

	// A menu holding a value previews the entry under the pointer, or else the one the arrow keys moved to, before either is committed
	function watchPreviewedEntry(entry: MenuListEntry | undefined) {
		if (!entry || entry.value === lastPreviewedValue) return;

		lastPreviewedValue = entry.value;
		dispatch("preview", entry);
	}

	function watchEntriesHash(_: bigint) {
		reactiveEntries = entries;
	}

	function watchRemeasureWidth(_: MenuListEntry[][], __: boolean, ___: FloatingMenu | undefined, measuresNaturalWidth: boolean, search: string) {
		if (!measuresNaturalWidth) return;

		// Skip re-measurement for virtual scrolling menus since ResizeObserver handles their width
		if (virtualScrolling) return;

		// A search shows only the matching entries, which would measure narrower than the whole menu
		if (search) return;

		self?.measureAndEmitNaturalWidth();
	}

	async function startMenuWidthObserver() {
		await tick();
		// Guard against the menu having closed during the tick
		if (!open) return;

		const floatingMenuContentDiv = contentElement();
		if (!floatingMenuContentDiv) return;

		maxMenuWidth = 0;

		resizeObserver?.disconnect();
		resizeObserver = new ResizeObserver(() => {
			const width = floatingMenuContentDiv.scrollWidth;
			if (width > maxMenuWidth) {
				maxMenuWidth = width;
				floatingMenuContentDiv.style.minWidth = `${maxMenuWidth}px`;
			}
		});
		resizeObserver.observe(floatingMenuContentDiv);
	}

	function onVirtualScroll(e: Event) {
		if (!virtualScrollingEntryHeight) return;
		virtualScrollingEntriesStart = e.target instanceof HTMLElement ? e.target.scrollTop : 0;
	}

	function contentElement(): HTMLElement | undefined {
		const content = self?.div()?.querySelector("[data-floating-menu-content]");
		return content instanceof HTMLElement ? content : undefined;
	}

	function inSearch(search: string, entry: MenuListEntry): boolean {
		return !search || entry.label.toLowerCase().includes(search.toLowerCase());
	}

	function selectableEntries(): MenuListEntry[] {
		return filteredEntries.flat().filter((entry) => !entry.disabled);
	}

	function openChildEntry(): MenuListEntry | undefined {
		if (openChildValue === undefined) return undefined;
		return filteredEntries.flat().find((entry) => entry.value === openChildValue);
	}

	// The submenu that's open, once it exists, rather than one still on its way out
	function currentChild(): MenuList | undefined {
		if (openChildValue === undefined || openChild?.parentsValuePath.at(-1) !== openChildValue) return undefined;
		return openChild;
	}

	function includeSeparator(entries: MenuListEntry[][], section: MenuListEntry[], sectionIndex: number, search: string): boolean {
		const elementsBeforeCurrentSection = entries
			.slice(0, sectionIndex)
			.flat()
			.filter((entry) => inSearch(search, entry));
		const entriesInCurrentSection = section.filter((entry) => inSearch(search, entry));

		return elementsBeforeCurrentSection.length > 0 && entriesInCurrentSection.length > 0;
	}

	function currentEntries(section: MenuListEntry[], virtualScrollingEntryHeight: number, virtualScrollingStartIndex: number, virtualScrollingEndIndex: number, search: string) {
		if (!virtualScrollingEntryHeight) {
			return section.filter((entry) => inSearch(search, entry));
		}
		return section.filter((entry) => inSearch(search, entry)).slice(virtualScrollingStartIndex, virtualScrollingEndIndex);
	}

	// Pointer input

	function onPointerMove(e: PointerEvent) {
		lastPointerPosition = { x: e.clientX, y: e.clientY };

		// Entries passing under the resting pointer, as when the arrow keys scroll the menu, leave the hover where the pointer last put it
		if (e.movementX === 0 && e.movementY === 0) return;

		if (arrowKeysOpenNeighborMenus) menuBarKeyboardLeads.set(false);
		hoverAtPointer(e.target || undefined);
	}

	function onWheel(e: WheelEvent) {
		if (!(e.target instanceof Node) || !self?.div()?.contains(e.target)) return;

		lastPointerPosition = { x: e.clientX, y: e.clientY };
		wheelScrolled = true;
	}

	// The entries the wheel scrolls under the resting pointer take the hover, as moving the pointer onto them would
	function onScrollWhileOpen(e: Event) {
		if (!wheelScrolled || !lastPointerPosition || !(e.target instanceof Node) || !self?.div()?.contains(e.target)) return;

		hoverAtPointer(document.elementFromPoint(lastPointerPosition.x, lastPointerPosition.y) || undefined);
	}

	function hoverAtPointer(target: EventTarget | undefined) {
		const element = target instanceof Element ? target : undefined;

		// The pointer moving within the menu takes over from the arrow keys, whose highlight in a menu of actions would otherwise misstate what Enter picks
		if (pointerOver(element) && !holdsValue) clearHighlights();
	}

	/// Updates this menu, then its open submenu, for the pointer having moved onto the given element, returning whether it moved within either.
	export function pointerOver(target: Element | undefined): boolean {
		const content = contentElement();
		const inThisMenu = content !== undefined && target?.closest("[data-floating-menu-content]") === content;
		const row = inThisMenu ? target?.closest("[data-menu-list-row]") : undefined;
		const entry = row && content?.contains(row) ? filteredEntries.flat().find((candidate) => candidate.value === row.getAttribute("data-menu-list-row")) : undefined;
		const pickable = entry && !entry.disabled ? entry : undefined;

		if (!inThisMenu) {
			hovered = undefined;

			// A highlight following the active entry, like a list of suggestions has, goes back to it once the pointer leaves
			if (highlightFollowsActiveEntry && !highlighted) highlighted = activeEntry;
		} else if (pickable) {
			// Reassigning it redraws every row, so it's left alone while the pointer stays on the same one
			if (hovered?.value !== pickable.value) hovered = pickable;
			pointerLed = true;

			// The submenu of the entry under the pointer opens without the keyboard moving into it, and any other closes
			openChildValue = pickable.children?.length ? pickable.value : undefined;
		} else if (hovered) {
			// Elsewhere in the menu, like on a separator, a disabled entry, or the padding, the hover stays on the entry the pointer came from
			pointerLed = true;
		}

		const movedHere = inThisMenu && hovered !== undefined;
		const movedInSubmenu = currentChild()?.pointerOver(target) || false;
		return movedHere || movedInSubmenu;
	}

	export function clearHighlights() {
		highlighted = undefined;
		currentChild()?.clearHighlights();
	}

	function onEntryClick(e: MouseEvent, entry: MenuListEntry) {
		// A click in a submenu also reaches the entry it's nested in, which isn't the one clicked
		if (!(e.target instanceof Element) || e.target.closest("[data-menu-list-row]") !== e.currentTarget) return;
		if (entry.disabled) return;

		// An entry with a submenu opens it, as hovering already will have, rather than being picked
		if (entry.children?.length) {
			openChildValue = entry.value;
			return;
		}

		pick(entry);
	}

	function pick(entry: MenuListEntry) {
		// Nothing is left to commit as the menu closes
		if (holdsValue) {
			highlighted = entry;
			valueWhenOpened = entry;
		}

		dispatch("activeEntry", entry);
		dispatch("selectedEntryValuePath", [...parentsValuePath, entry.value]);

		if (!submenu) open = false;
	}

	function onSubmenuPick(valuePath: string[]) {
		dispatch("selectedEntryValuePath", valuePath);

		if (!submenu) open = false;
	}

	// Keyboard input

	/// Handles a key pressed while this top-level menu is open, in whichever of its submenus the keyboard is in.
	export function keydown(e: KeyboardEvent) {
		wheelScrolled = false;

		const outcome = routeKey(e);
		if (arrowKeysOpenNeighborMenus && outcome !== "Unhandled") menuBarKeyboardLeads.set(true);

		if ((outcome === "NextMenu" || outcome === "PreviousMenu") && arrowKeysOpenNeighborMenus) {
			e.preventDefault();
			openNeighborMenu(outcome === "NextMenu" ? 1 : -1);
		}
	}

	/// Handles a key in the open submenu if the keyboard or pointer has moved into it, or else in this menu.
	export function routeKey(e: KeyboardEvent): KeyOutcome {
		const child = currentChild();
		if (child?.hasPosition()) {
			const outcome = child.routeKey(e);
			if (outcome !== "CloseSubmenu") return outcome;

			// Back in this menu, on the entry whose submenu closed
			const entry = openChildEntry();
			openChildValue = undefined;
			highlightFromKeys(entry);
			focusSearch();
			return "Handled";
		}

		return handleKey(e);
	}

	/// Whether the arrow keys, the pointer, or a search have moved into this menu, so it takes the keys rather than the menu it's nested in.
	export function hasPosition(): boolean {
		return Boolean(highlighted || hovered || search);
	}

	/// Moves the keyboard into this menu at its first entry.
	export function highlightFirstEntry() {
		highlightFromKeys(selectableEntries()[0]);
	}

	/// Picks the highlighted entry, or the one under the pointer if it moved there since, as clicking it would, returning whether there was one.
	export function pickHighlighted(): boolean {
		const entry = keyboardStart();
		if (!open || !entry || entry.disabled) return false;

		if (entry.children?.length) enterSubmenu(entry);
		else pick(entry);
		return true;
	}

	// The entry the keys act on and move from: the one under the pointer if it moved there since the keys were last used, or else the highlight
	function keyboardStart(): MenuListEntry | undefined {
		if (pointerLed && hovered) return hovered;
		return highlighted || hovered || openChildEntry();
	}

	function handleKey(e: KeyboardEvent): KeyOutcome {
		const searchFocused = Boolean(search) && searchTextInput?.element() === document.activeElement;
		const start = keyboardStart();
		const startOpensSubmenu = Boolean(start && !start.disabled && start.children?.length);

		// With a modifier held, keys don't navigate, though AltGr may still type a character
		if ((e.ctrlKey || e.metaKey || e.altKey) && !e.getModifierState("AltGraph")) return typeIntoSearch(e, searchFocused);

		if (e.key === "ArrowDown" || e.key === "ArrowUp") return moveHighlight(e, e.key === "ArrowDown" ? 1 : -1, true);
		if (e.key === "Tab") return moveHighlight(e, e.shiftKey ? -1 : 1, true);
		if (e.key === "PageDown" || e.key === "PageUp") return moveHighlight(e, (e.key === "PageDown" ? 1 : -1) * pageEntryCount(), false);
		if ((e.key === "Home" || e.key === "End") && !searchFocused) return moveHighlight(e, e.key === "End" ? Infinity : -Infinity, false);

		if (e.key === "ArrowRight") {
			if (start && startOpensSubmenu) return enterSubmenu(start, e);
			if (searchFocused) return "Unhandled";
			return "NextMenu";
		}

		if (e.key === "ArrowLeft") {
			if (searchFocused) return "Unhandled";
			e.preventDefault();
			return submenu ? "CloseSubmenu" : "PreviousMenu";
		}

		if (e.key === "Escape") {
			e.preventDefault();
			if (submenu) return "CloseSubmenu";

			// A menu holding a value goes back to the one it opened with, and a floating menu it's in stays open
			if (holdsValue) highlighted = valueWhenOpened;
			const element = self?.div();
			if (element) preventEscapeClosingParentFloatingMenu(element);

			open = false;
			return "Handled";
		}

		if (e.key === "Enter" || (e.key === " " && !searchFocused)) {
			e.preventDefault();

			if (start && startOpensSubmenu) return enterSubmenu(start, e);
			if (start && !start.disabled) pick(start);
			// With nothing highlighted, a menu holding a value closes with whichever value it has
			else if (holdsValue) open = false;
			return "Handled";
		}

		if (e.key === "Backspace" && search !== "" && !searchFocused) {
			e.preventDefault();
			search = search.slice(0, -1);
			focusSearch();
			return "Handled";
		}

		return typeIntoSearch(e, searchFocused);
	}

	// Moves the highlight by a number of entries, wrapping around between the ends unless the menu holds a value or it moves by more than one
	function moveHighlight(e: KeyboardEvent, step: number, wrapping: boolean): KeyOutcome {
		e.preventDefault();

		const choices = selectableEntries();
		if (choices.length === 0) return "Handled";

		const current = keyboardStart();
		const currentIndex = current ? choices.findIndex((entry) => entry.value === current.value) : -1;

		let index;
		if (!Number.isFinite(step)) index = step > 0 ? choices.length - 1 : 0;
		else if (currentIndex === -1) index = step > 0 ? 0 : choices.length - 1;
		else if (wrapping && !holdsValue) index = (currentIndex + step + choices.length) % choices.length;
		else index = Math.min(Math.max(currentIndex + step, 0), choices.length - 1);

		highlightFromKeys(choices[index]);
		return "Handled";
	}

	function pageEntryCount(): number {
		const scrolling = virtualScrollingEntryHeight ? scroller?.div?.() : contentElement();
		return Math.max(1, Math.floor((scrolling?.clientHeight || 0) / ENTRY_HEIGHT) - 1);
	}

	function highlightFromKeys(entry: MenuListEntry | undefined) {
		pointerLed = false;

		// The arrow keys pass over entries without opening their submenus, so one left behind closes
		if (openChildValue !== undefined && openChildValue !== entry?.value) openChildValue = undefined;

		setHighlighted(entry);
	}

	function enterSubmenu(entry: MenuListEntry, e?: KeyboardEvent): KeyOutcome {
		e?.preventDefault();

		openChildValue = entry.value;
		highlightFromKeys(entry);
		searchTextInput?.element()?.blur();

		// Once the submenu exists, the keyboard moves into it at its first entry
		tick().then(() => currentChild()?.highlightFirstEntry());

		return "Handled";
	}

	// Types a character into this menu's search, creating its search box if it has none yet
	function typeIntoSearch(e: KeyboardEvent, searchFocused: boolean): KeyOutcome {
		// Only single printable characters search, and a space only once a search has begun
		if (e.key.length !== 1 || (e.key === " " && !search)) return "Unhandled";

		// Nor a shortcut's key, though AltGr, which also reports Ctrl, types characters
		if ((e.ctrlKey || e.metaKey) && !e.getModifierState("AltGraph")) {
			// The browser's own action for it, like saving the page, stays held back, except a text field's editing shortcuts
			if (!targetIsTextField(e.target || undefined)) e.preventDefault();
			return "Unhandled";
		}
		if (e.altKey && !e.getModifierState("AltGraph")) return "Unhandled";

		// Once focused, the search box takes the typing itself
		if (searchFocused) return "Unhandled";

		e.preventDefault();
		pointerLed = false;
		search += e.key;
		focusSearch();

		return "Handled";
	}

	// Focuses this menu's search box, if it has one, with the cursor at the end, so typing continues there
	async function focusSearch() {
		// Must wait until the search box exists, after the search begins
		await tick();

		const searchElement = searchTextInput?.element();
		if (!searchTextInput || !searchElement) return;

		searchTextInput.focus();
		searchElement.setSelectionRange(search.length, search.length);
	}

	// Moves from this menu to the one beside it among those hovering moves between, like the menu bar's, which opens on its first entry
	function openNeighborMenu(step: number) {
		const spawner = self?.div()?.parentElement?.querySelector(":scope > [data-floating-menu-spawner]");
		const group = spawner?.parentElement?.parentElement;
		if (!spawner || !group) return;

		const neighbors = Array.from(group.querySelectorAll(":scope > * > [data-floating-menu-spawner]:not([data-block-hover-transfer])"));
		const index = neighbors.indexOf(spawner);
		const neighbor = neighbors[(index + step + neighbors.length) % neighbors.length];
		if (index === -1 || neighbor === spawner || !(neighbor instanceof HTMLElement)) return;

		open = false;
		menuOpenedByArrowKeys.set(true);
		neighbor.click();

		// The neighbor clears the flag as it opens, so a flag still set means none opened
		tick().then(() => {
			menuBarKeyboardLeads.set(!get(menuOpenedByArrowKeys));
			menuOpenedByArrowKeys.set(false);
		});
	}

	function setHighlighted(newHighlight: MenuListEntry | undefined) {
		highlighted = newHighlight;

		// Scroll into view
		let container = scroller?.div?.();
		if (!container || !highlighted) return;
		const highlightedIndex = filteredEntries.flat().findIndex((entry) => entry.value === highlighted?.value);

		// Without virtual scrolling, every row exists and the floating menu around them is what scrolls, so the row is brought into its view
		if (!virtualScrollingEntryHeight) {
			const rows = Array.from(container.querySelectorAll(":scope > [data-menu-list-row]"));
			rows[highlightedIndex - startIndex]?.scrollIntoView({ block: "nearest" });
			return;
		}

		const containerBoundingRect = container.getBoundingClientRect();
		const selectedBoundingRect = new DOMRect();
		selectedBoundingRect.y = highlightedIndex * virtualScrollingEntryHeight - container.scrollTop + containerBoundingRect.y;
		selectedBoundingRect.height = virtualScrollingEntryHeight;

		if (containerBoundingRect.y > selectedBoundingRect.y) {
			container.scrollBy(0, selectedBoundingRect.y - containerBoundingRect.y);
		}
		if (containerBoundingRect.y + containerBoundingRect.height < selectedBoundingRect.y + selectedBoundingRect.height) {
			container.scrollBy(0, selectedBoundingRect.y - (containerBoundingRect.y + containerBoundingRect.height) + selectedBoundingRect.height);
		}
	}
</script>

<FloatingMenu
	class="menu-list"
	{open}
	on:open={({ detail }) => (open = detail)}
	on:naturalWidth
	type="Dropdown"
	windowEdgeMargin={0}
	escapeCloses={false}
	{direction}
	{minWidth}
	scrollableY={scrollableY && virtualScrollingEntryHeight === 0}
	bind:this={self}
>
	{#if search.length > 0}
		<TextInput class="search" value={search} on:value={({ detail }) => (search = detail)} bind:this={searchTextInput}></TextInput>
	{/if}
	<!-- If we put the scrollableY on the layoutcol for non-font dropdowns then for some reason it always creates a tiny scrollbar.
	However when we are using the virtual scrolling then we need the layoutcol to be scrolling so we can bind the events without using `self`. -->
	<LayoutCol
		bind:this={scroller}
		scrollableY={scrollableY && virtualScrollingEntryHeight !== 0}
		on:scroll={onVirtualScroll}
		styles={{ "min-width": virtualScrollingEntryHeight ? `${minWidth}px` : `inherit` }}
	>
		{#if virtualScrollingEntryHeight}
			<LayoutRow class="scroll-spacer" styles={{ height: `${virtualScrollingStartIndex * virtualScrollingEntryHeight}px` }} />
		{/if}
		{#each reactiveEntries as section, sectionIndex (sectionIndex)}
			{#if includeSeparator(reactiveEntries, section, sectionIndex, search)}
				<Separator style="Section" direction="Vertical" />
			{/if}
			{#each currentEntries(section, virtualScrollingEntryHeight, virtualScrollingStartIndex, virtualScrollingEndIndex, search) as entry, entryIndex (entryIndex + startIndex)}
				<LayoutRow
					class="row"
					classes={{
						open: openChildValue === entry.value,
						// Hidden once the arrow keys take over, until the pointer moves again
						hovered: pointerLed && entry.value === hovered?.value,
						// The keyboard is in the open submenu, where its own highlight is the only one drawn
						active: entry.value === highlighted?.value && openChildValue !== entry.value,
						disabled: Boolean(entry.disabled),
					}}
					styles={{ height: virtualScrollingEntryHeight || `${ENTRY_HEIGHT}px` }}
					tooltipLabel={entry.tooltipLabel}
					tooltipDescription={entry.tooltipDescription}
					tooltipShortcut={entry.tooltipShortcut}
					tooltipCode={entry.tooltipCode}
					on:click={(e) => onEntryClick(e, entry)}
					data-menu-list-row={entry.value}
				>
					{#if entry.icon && drawIcon}
						<IconLabel icon={entry.icon} iconSizeOverride={16} class="entry-icon" />
					{:else if drawIcon}
						<div class="no-icon"></div>
					{/if}

					{#if entry.font}
						<link
							rel="stylesheet"
							href={entry.font}
							onload={() => {
								document.fonts.load(`16px "${entry.value}"`).then(() => {
									loadedFonts.add(entry.value);
									loadedFontsGeneration += 1; // Modify the dirty trigger
								});
							}}
						/>
					{/if}

					<TextLabel
						class="entry-label"
						{monospace}
						classes={{
							"font-preview": Boolean(entry.font),
							"font-loaded": loadedFontsGeneration >= 0 && loadedFonts.has(entry.value),
						}}
						styles={entry.font ? { "font-family": `"${entry.value}", "Source Sans Pro"` } : {}}
					>
						{entry.label}
					</TextLabel>

					{#if entry.tooltipShortcut?.shortcut.length}
						<ShortcutLabel shortcut={entry.tooltipShortcut} />
					{:else if entry.annotation}
						<TextLabel class="entry-annotation">{entry.annotation}</TextLabel>
					{/if}

					{#if entry.children?.length}
						<IconLabel class="submenu-arrow" icon="DropdownArrow" />
					{:else if !entry.annotation || entry.tooltipShortcut?.shortcut.length}
						<div class="no-submenu-arrow"></div>
					{/if}

					{#if entry.children?.length && openChildValue === entry.value}
						<MenuList
							on:selectedEntryValuePath={({ detail }) => onSubmenuPick(detail)}
							parentsValuePath={[...parentsValuePath, entry.value]}
							open={true}
							submenu={true}
							direction="TopRight"
							entries={entry.children}
							entriesHash={entry.childrenHash || 0n}
							{minWidth}
							{drawIcon}
							{monospace}
							{scrollableY}
							bind:this={openChild}
						/>
					{/if}
				</LayoutRow>
			{/each}
		{/each}
		{#if virtualScrollingEntryHeight}
			<LayoutRow class="scroll-spacer" styles={{ height: `${virtualScrollingTotalHeight - virtualScrollingEndIndex * virtualScrollingEntryHeight}px` }} />
		{/if}
	</LayoutCol>
</FloatingMenu>

<style lang="scss">
	.menu-list {
		.search {
			margin: 4px;
			margin-top: 0;
		}

		.floating-menu-container .floating-menu-content.floating-menu-content {
			padding: 4px 0;
			// A row scrolled into view at either end keeps the padding beyond it in view too
			scroll-padding: 4px 0;

			// Where the menu itself scrolls, its entries keep their full height so the padding after them is reached too
			> .layout-col:not(.scrollable-y) {
				flex-shrink: 0;
			}

			.separator {
				margin: 4px 0;

				div {
					background: var(--color-3-darkgray);
				}
			}

			.scroll-spacer {
				flex: 0 0 auto;
			}

			.row {
				height: 20px;
				align-items: center;
				white-space: nowrap;
				position: relative;
				flex: 0 0 auto;
				border-radius: 2px;
				margin: 0 4px;

				> * {
					flex: 0 0 auto;
				}

				.no-icon {
					width: 16px;
					height: 16px;
				}

				.entry-label {
					flex: 1 1 100%;
					margin: 0 4px;
				}

				.font-preview:not(.font-loaded) {
					opacity: 0.5;
				}

				.entry-icon,
				.no-icon {
					margin: 0 4px;
				}

				.shortcut-label,
				.entry-annotation {
					margin-left: 12px;
				}

				// Styled like a shortcut's keys, which it takes the place of, but inset from the row's end like the label from its start
				.entry-annotation {
					margin-right: 4px;
					color: var(--color-8-uppergray);
				}

				.submenu-arrow {
					transform: rotate(270deg);
				}

				.no-submenu-arrow {
					width: 12px;
					height: 12px;
				}

				// Extend the submenu to the right by the width of the margin outside the row, since we want the submenu to line up with the edge of the menu
				&.open {
					// Offset by the margin distance
					> .menu-list {
						margin-right: -4px;
					}

					// Extend the click target by the margin distance so the user can hover to the right of the row, within the margin area, and still have the submenu open
					&::after {
						content: "";
						position: absolute;
						top: 0;
						right: -4px;
						width: 4px;
						height: 100%;
					}
				}

				&.hovered,
				&.open {
					background: var(--color-4-dimgray);
				}

				&.active {
					background: var(--color-e-nearwhite);
					color: var(--color-2-mildblack);

					> .icon-label {
						fill: var(--color-2-mildblack);
					}
				}

				&.disabled {
					color: var(--color-8-uppergray);

					svg {
						fill: var(--color-8-uppergray);
					}
				}
			}
		}
	}
	// paddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpadding
</style>
