<script lang="ts">
	import { tick, onDestroy, onMount } from "svelte";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import UserInputLabel from "@graphite/components/widgets/labels/UserInputLabel.svelte";
	import type { MenuListEntry, MenuDirection } from "@graphite/messages.svelte";

	let self: FloatingMenu | undefined = $state();
	let scroller: LayoutCol | undefined = $state();
	let searchTextInput: TextInput | undefined = $state();

	type Props = {
		entries: MenuListEntry[][];
		activeEntry?: MenuListEntry | undefined;
		open: boolean;
		direction?: MenuDirection;
		minWidth?: number;
		drawIcon?: boolean;
		interactive?: boolean;
		scrollableY?: boolean;
		virtualScrollingEntryHeight?: number;
		tooltip?: string | undefined;
		onhoverOutEntry?: () => void;
		onhoverInEntry?: (entry: MenuListEntry) => void;
		onnaturalWidth?: (width: number) => void;
		onactiveEntry?: (activeEntry: MenuListEntry) => void;
	};

	let {
		entries = $bindable(),
		activeEntry = undefined,
		open = $bindable(false),
		direction = "Bottom",
		minWidth = 0,
		drawIcon = false,
		interactive = false,
		scrollableY = false,
		virtualScrollingEntryHeight = 0,
		tooltip = undefined,
		onhoverOutEntry,
		onhoverInEntry,
		onnaturalWidth,
		onactiveEntry,
	}: Props = $props();

	// Keep the child references outside of the entries array so as to avoid infinite recursion.
	let childReferences: MenuList[][] = $state([]);
	let search = $state("");

	let highlighted = $state(activeEntry as MenuListEntry | undefined);
	let virtualScrollingEntriesStart = $state(0);

	// TODO: Move keyboard input handling entirely to the unified system in `input.ts`.
	// TODO: The current approach is hacky and blocks the allowances for shortcuts like the key to open the browser's dev tools.
	onMount(async () => {
		await tick();
		if (open && !inNestedMenuList()) addEventListener("keydown", keydown);
	});
	onDestroy(async () => {
		await tick();
		if (!inNestedMenuList()) removeEventListener("keydown", keydown);
	});

	function inNestedMenuList(): boolean {
		const div = self?.div();
		if (!(div instanceof HTMLDivElement)) return false;
		return Boolean(div.closest("[data-floating-menu-content]"));
	}

	// Required to keep the highlighted item centered and to find a new highlighted item if necessary
	async function watchHighlightedWithSearch(filteredEntries: MenuListEntry[][], open: boolean) {
		if (highlighted && open) {
			// Allows the scrollable area to expand if necessary
			await tick();

			const flattened = filteredEntries.flat();
			const highlightedFound = highlighted?.label && flattened.map((entry) => entry.label).includes(highlighted.label);
			const newHighlighted = highlightedFound ? highlighted : flattened[0];
			setHighlighted(newHighlighted);
		}
	}

	// Detect when the user types, which creates a search box
	async function startSearch(e: KeyboardEvent) {
		// Only accept single-character symbol inputs other than space
		if (e.key.length !== 1 || e.key === " ") return;

		// Stop shortcuts being activated
		e.stopPropagation();
		e.preventDefault();

		// Forward the input's first character to the search box, which after that point the user will continue typing into directly
		search = e.key;

		// Must wait until the DOM elements have been created (after the if condition becomes true) before the search box exists
		await tick();

		// Get the search box element
		const searchElement = searchTextInput?.element();
		if (!searchTextInput || !searchElement) return;

		// Focus the search box and move the cursor to the end
		searchTextInput.focus();
		searchElement.setSelectionRange(search.length, search.length);

		// Continue listening for keyboard navigation even when the search box is focused
		// searchElement.onkeydown = (e) => {
		// 	if (["Enter", "Escape", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
		// 		keydown(e, false);
		// 	}
		// };
	}

	function inSearch(search: string, entry: MenuListEntry): boolean {
		return !search || entry.label.toLowerCase().includes(search.toLowerCase());
	}

	function watchOpen(value: boolean) {
		if (value && !inNestedMenuList()) addEventListener("keydown", keydown);
		else if (!inNestedMenuList()) removeEventListener("keydown", keydown);

		highlighted = activeEntry;
		// dispatch("open", value);
		// open = value;

		search = "";
	}

	function watchEntries(entries: MenuListEntry[][]) {
		entries.forEach((_, index) => {
			if (!childReferences[index]) childReferences[index] = [];
		});
	}

	function watchRemeasureWidth(_: MenuListEntry[][], __: boolean) {
		self?.measureAndEmitNaturalWidth();
	}

	function onScroll(e: Event) {
		if (!virtualScrollingEntryHeight) return;
		virtualScrollingEntriesStart = (e.target as HTMLElement)?.scrollTop || 0;
	}

	function getChildReference(menuListEntry: MenuListEntry): MenuList | undefined {
		const index = filteredEntries.flat().indexOf(menuListEntry);
		return childReferences.flat().filter((x) => x)[index];
	}

	function onEntryClick(menuListEntry: MenuListEntry) {
		// Call the action if available
		if (menuListEntry.action) menuListEntry.action();

		// Notify the parent about the clicked entry as the new active entry
		onactiveEntry?.(menuListEntry);

		// Close the containing menu
		let childReference = getChildReference(menuListEntry);
		if (childReference) {
			childReference.open = false;
			// entries = entries;
		}
		// dispatch("open", false);
		open = false;
	}

	function onEntryPointerEnter(menuListEntry: MenuListEntry) {
		if (!menuListEntry.children?.length) {
			onhoverInEntry?.(menuListEntry);
			return;
		}

		let childReference = getChildReference(menuListEntry);
		if (childReference) {
			childReference.open = true;
			// entries = entries;
		} else open = true;
	}

	function onEntryPointerLeave(menuListEntry: MenuListEntry) {
		if (!menuListEntry.children?.length) {
			// dispatch("hoverOutEntry");
			onhoverOutEntry?.();
			return;
		}

		let childReference = getChildReference(menuListEntry);
		if (childReference) {
			childReference.open = false;
			// entries = entries;
		} else open = false;
	}

	function isEntryOpen(menuListEntry: MenuListEntry): boolean {
		if (!menuListEntry.children?.length) return false;

		return getChildReference(menuListEntry)?.open || false;
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

	function openSubmenu(highlightedEntry: MenuListEntry): boolean {
		let childReference = getChildReference(highlightedEntry);
		// No submenu to open
		if (!childReference || !highlightedEntry.children?.length) return false;

		childReference.open = true;
		// The reason we bother taking `highlightdEntry` as an argument is because, when this function is called, it can ensure `highlightedEntry` is not undefined.
		// But here we still have to set `highlighted` to itself so Svelte knows to reactively update it after we set its `childReference.open` property.
		highlighted = highlighted;

		// Highlight first item
		childReference.setHighlighted(highlightedEntry.children[0][0]);

		// Submenu was opened
		return true;
	}

	/// Handles keyboard navigation for the menu.
	// Returns a boolean indicating whether the entire menu stack should be dismissed.
	export function keydown(e: KeyboardEvent, submenu = false): boolean {
		// Interactive menus should keep the active entry the same as the highlighted one
		if (interactive) highlighted = activeEntry;

		const menuOpen = open;
		const flatEntries = filteredEntries.flat().filter((entry) => !entry.disabled);
		const openChild = flatEntries.findIndex((entry) => (entry.children?.length ?? 0) > 0 && getChildReference(entry)?.open);

		// Allow opening menu with space or enter
		if (!menuOpen && (e.key === " " || e.key === "Enter")) {
			open = true;
			highlighted = activeEntry;

			// Keep the menu stack open
			return false;
		}

		// If a submenu is open, have it handle this instead
		if (menuOpen && openChild >= 0) {
			const childMenuListEntry = flatEntries[openChild];
			const childMenu = getChildReference(childMenuListEntry);

			// Redirect the keyboard navigation to a submenu if one is open
			const shouldCloseStack = childMenu?.keydown(e, true) || false;

			// Highlight the menu item in the parent list that corresponds with the open submenu
			if (highlighted && e.key !== "Escape") setHighlighted(childMenuListEntry);

			// Handle the child closing the entire menu stack
			if (shouldCloseStack) open = false;

			// Keep the menu stack open
			return shouldCloseStack;
		}

		// Navigate to the next and previous entries with arrow keys
		if ((menuOpen || interactive) && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
			let newIndex = e.key === "ArrowUp" ? flatEntries.length - 1 : 0;
			if (highlighted) {
				const index = highlighted ? flatEntries.map((entry) => entry.label).indexOf(highlighted.label) : 0;
				newIndex = index + (e.key === "ArrowUp" ? -1 : 1);

				// Interactive dropdowns should lock at the end whereas other dropdowns should loop
				if (interactive) newIndex = Math.min(flatEntries.length - 1, Math.max(0, newIndex));
				else newIndex = (newIndex + flatEntries.length) % flatEntries.length;
			}

			const newEntry = flatEntries[newIndex];
			setHighlighted(newEntry);

			e.preventDefault();

			// Keep the menu stack open
			return false;
		}

		// Close menu with escape key
		if (menuOpen && e.key === "Escape") {
			open = false;

			// Reset active to before open
			setHighlighted(activeEntry);

			// Keep the menu stack open
			return false;
		}

		// Click on a highlighted entry with the enter key
		if (menuOpen && highlighted && e.key === "Enter") {
			// Handle clicking on an option if enter is pressed
			if (!highlighted.children?.length) onEntryClick(highlighted);
			else openSubmenu(highlighted);

			// Stop the event from triggering a press on a new dialog
			e.preventDefault();

			// Enter should close the entire menu stack
			return true;
		}

		// Open a submenu with the right arrow key, space, or enter
		if (menuOpen && highlighted && (e.key === "ArrowRight" || e.key === " " || e.key === "Enter")) {
			// Right arrow opens a submenu
			const openable = openSubmenu(highlighted);

			// Prevent the right arrow from moving the search text cursor if we are opening a submenu
			if (openable) e.preventDefault();

			// Keep the menu stack open
			return false;
		}

		// Close a submenu with the left arrow key
		if (menuOpen && e.key === "ArrowLeft") {
			// Left arrow closes a submenu
			if (submenu) {
				open = false;

				e.preventDefault();
			}

			// Keep the menu stack open
			return false;
		}

		// Start a search with any other key
		if (menuOpen && search === "") {
			startSearch(e);

			// Keep the menu stack open
			return false;
		}

		// If nothing happened, keep the menu stack open
		return false;
	}

	export function setHighlighted(newHighlight: MenuListEntry | undefined) {
		highlighted = newHighlight;

		// Interactive menus should keep the active entry the same as the highlighted one
		// if (interactive && newHighlight?.value !== activeEntry?.value && newHighlight) {
		// 	dispatch("activeEntry", newHighlight);
		// }

		// Scroll into view
		let container = scroller?.div?.();
		if (!container || !highlighted) return;
		let containerBoundingRect = container.getBoundingClientRect();
		let highlightedIndex = filteredEntries.flat().findIndex((entry) => entry === highlighted);

		let selectedBoundingRect = new DOMRect();
		if (virtualScrollingEntryHeight) {
			// Special case for virtual scrolling
			selectedBoundingRect.y = highlightedIndex * virtualScrollingEntryHeight - container.scrollTop + containerBoundingRect.y;
			selectedBoundingRect.height = virtualScrollingEntryHeight;
		} else {
			let entries = Array.from(container.children).filter((element) => element.classList.contains("row"));
			let element = entries[highlightedIndex - startIndex];
			if (!element) return;
			containerBoundingRect = element.getBoundingClientRect();
		}

		if (containerBoundingRect.y > selectedBoundingRect.y) {
			container.scrollBy(0, selectedBoundingRect.y - containerBoundingRect.y);
		}
		if (containerBoundingRect.y + containerBoundingRect.height < selectedBoundingRect.y + selectedBoundingRect.height) {
			container.scrollBy(0, selectedBoundingRect.y - (containerBoundingRect.y + containerBoundingRect.height) + selectedBoundingRect.height);
		}
	}

	export function scrollViewTo(distanceDown: number) {
		scroller?.div?.()?.scrollTo(0, distanceDown);
	}
	// Called only when `open` is changed from outside this component
	$effect(() => {
		watchOpen(open);
	});
	$effect(() => {
		watchEntries(entries);
	});
	let filteredEntries = $derived(entries.map((section) => section.filter((entry) => inSearch(search, entry))));
	$effect(() => {
		watchRemeasureWidth(filteredEntries, drawIcon);
	});
	$effect(() => {
		watchHighlightedWithSearch(filteredEntries, open);
	});
	let virtualScrollingTotalHeight = $derived(filteredEntries.length === 0 ? 0 : filteredEntries[0].length * virtualScrollingEntryHeight);
	let virtualScrollingStartIndex = $derived(Math.floor(virtualScrollingEntriesStart / virtualScrollingEntryHeight) || 0);
	let virtualScrollingEndIndex = $derived(filteredEntries.length === 0 ? 0 : Math.min(filteredEntries[0].length, virtualScrollingStartIndex + 1 + 400 / virtualScrollingEntryHeight));
	let startIndex = $derived(virtualScrollingEntryHeight ? virtualScrollingStartIndex : 0);

	export { entries, activeEntry, open, direction, minWidth, drawIcon, interactive, scrollableY, virtualScrollingEntryHeight, tooltip };
</script>

<FloatingMenu
	class="menu-list"
	bind:open
	type="Dropdown"
	windowEdgeMargin={0}
	escapeCloses={false}
	{direction}
	{minWidth}
	scrollableY={scrollableY && virtualScrollingEntryHeight === 0}
	{onnaturalWidth}
	bind:this={self}
>
	{#if search.length > 0}
		<TextInput class="search" bind:value={search} bind:this={searchTextInput}></TextInput>
	{/if}
	<!-- If we put the scrollableY on the layoutcol for non-font dropdowns then for some reason it always creates a tiny scrollbar.
	However when we are using the virtual scrolling then we need the layoutcol to be scrolling so we can bind the events without using `self`. -->
	<LayoutCol
		bind:this={scroller}
		scrollableY={scrollableY && virtualScrollingEntryHeight !== 0}
		onscroll={onScroll}
		styles={{ "min-width": virtualScrollingEntryHeight ? `${minWidth}px` : `inherit` }}
	>
		{#if virtualScrollingEntryHeight}
			<LayoutRow class="scroll-spacer" styles={{ height: `${virtualScrollingStartIndex * virtualScrollingEntryHeight}px` }} />
		{/if}
		{#each entries as section, sectionIndex (sectionIndex)}
			{#if includeSeparator(entries, section, sectionIndex, search)}
				<Separator type="Section" direction="Vertical" />
			{/if}
			{#each currentEntries(section, virtualScrollingEntryHeight, virtualScrollingStartIndex, virtualScrollingEndIndex, search) as entry, entryIndex (entryIndex + startIndex)}
				<LayoutRow
					class="row"
					classes={{ open: isEntryOpen(entry), active: entry.label === highlighted?.label, disabled: Boolean(entry.disabled) }}
					styles={{ height: virtualScrollingEntryHeight || "20px" }}
					{tooltip}
					onclick={() => !entry.disabled && onEntryClick(entry)}
					onpointerenter={() => !entry.disabled && onEntryPointerEnter(entry)}
					onpointerleave={() => !entry.disabled && onEntryPointerLeave(entry)}
				>
					{#if entry.icon && drawIcon}
						<IconLabel icon={entry.icon} iconSizeOverride={16} class="entry-icon" />
					{:else if drawIcon}
						<div class="no-icon"></div>
					{/if}

					{#if entry.font}
						<link rel="stylesheet" href={entry.font?.toString()} />
					{/if}

					<TextLabel class="entry-label" styles={{ "font-family": `${!entry.font ? "inherit" : entry.value}` }}>{entry.label}</TextLabel>

					{#if entry.shortcut?.keys.length}
						<UserInputLabel keysWithLabelsGroups={[entry.shortcut.keys]} requiresLock={entry.shortcutRequiresLock} textOnly={true} />
					{/if}

					{#if entry.children?.length}
						<IconLabel class="submenu-arrow" icon="DropdownArrow" />
					{:else}
						<div class="no-submenu-arrow"></div>
					{/if}

					{#if entry.children}
						<MenuList
							{onnaturalWidth}
							open={entry?.open ?? false}
							direction="TopRight"
							entries={entry.children}
							{minWidth}
							{drawIcon}
							{scrollableY}
							bind:this={childReferences[sectionIndex][entryIndex + startIndex]}
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

<style lang="scss" global>
	.menu-list {
		.search {
			margin: 4px;
			margin-top: 0;
		}

		.floating-menu-container .floating-menu-content.floating-menu-content {
			padding: 4px 0;

			.separator {
				margin: 4px 0;

				div {
					background: var(--color-4-dimgray);
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

				.entry-icon,
				.no-icon {
					margin: 0 4px;
				}

				.user-input-label {
					margin-left: 12px;
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

				&:hover,
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

					&:hover {
						background: none;
					}

					svg {
						fill: var(--color-8-uppergray);
					}
				}
			}
		}
	}
	// paddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpadding
</style>
