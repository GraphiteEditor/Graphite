<svelte:options accessors={true} />

<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { MenuListEntry } from "@graphite/wasm-communication/messages";

	import FloatingMenu, { type MenuDirection } from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import UserInputLabel from "@graphite/components/widgets/labels/UserInputLabel.svelte";

	let self: FloatingMenu | undefined;
	let scroller: LayoutCol | undefined;

	const dispatch = createEventDispatcher<{ open: boolean; activeEntry: MenuListEntry }>();

	export let entries: MenuListEntry[][];
	export let activeEntry: MenuListEntry | undefined = undefined;
	export let open: boolean;
	export let direction: MenuDirection = "Bottom";
	export let minWidth = 0;
	export let drawIcon = false;
	export let interactive = false;
	export let scrollableY = false;
	export let virtualScrollingEntryHeight = 0;
	export let tooltip: string | undefined = undefined;

	let highlighted = activeEntry as MenuListEntry | undefined;
	let virtualScrollingEntriesStart = 0;

	// Called only when `open` is changed from outside this component
	$: watchOpen(open);
	$: watchRemeasureWidth(entries, drawIcon);

	$: virtualScrollingTotalHeight = entries.length === 0 ? 0 : entries[0].length * virtualScrollingEntryHeight;
	$: virtualScrollingStartIndex = Math.floor(virtualScrollingEntriesStart / virtualScrollingEntryHeight) || 0;
	$: virtualScrollingEndIndex = entries.length === 0 ? 0 : Math.min(entries[0].length, virtualScrollingStartIndex + 1 + 400 / virtualScrollingEntryHeight);
	$: startIndex = virtualScrollingEntryHeight ? virtualScrollingStartIndex : 0;

	function watchOpen(open: boolean) {
		highlighted = activeEntry;
		dispatch("open", open);
	}

	function watchRemeasureWidth(_: MenuListEntry[][], __: boolean) {
		self?.measureAndEmitNaturalWidth();
	}

	function onScroll(e: Event) {
		if (!virtualScrollingEntryHeight) return;
		virtualScrollingEntriesStart = (e.target as HTMLElement)?.scrollTop || 0;
	}

	function onEntryClick(menuListEntry: MenuListEntry) {
		// Call the action if available
		if (menuListEntry.action) menuListEntry.action();

		// Notify the parent about the clicked entry as the new active entry
		dispatch("activeEntry", menuListEntry);

		// Close the containing menu
		if (menuListEntry.ref) {
			menuListEntry.ref.open = false;
			entries = entries;
		}
		dispatch("open", false);
		open = false;
	}

	function onEntryPointerEnter(menuListEntry: MenuListEntry) {
		if (!menuListEntry.children?.length) return;

		if (menuListEntry.ref) {
			menuListEntry.ref.open = true;
			entries = entries;
		} else dispatch("open", true);
	}

	function onEntryPointerLeave(menuListEntry: MenuListEntry) {
		if (!menuListEntry.children?.length) return;

		if (menuListEntry.ref) {
			menuListEntry.ref.open = false;
			entries = entries;
		} else dispatch("open", false);
	}

	function isEntryOpen(menuListEntry: MenuListEntry): boolean {
		if (!menuListEntry.children?.length) return false;

		return menuListEntry.ref?.open || false;
	}

	/// Handles keyboard navigation for the menu. Returns if the entire menu stack should be dismissed
	export function keydown(e: KeyboardEvent, submenu: boolean): boolean {
		// Interactive menus should keep the active entry the same as the highlighted one
		if (interactive) highlighted = activeEntry;

		const menuOpen = open;
		const flatEntries = entries.flat().filter((entry) => !entry.disabled);
		const openChild = flatEntries.findIndex((entry) => (entry.children?.length ?? 0) > 0 && entry.ref?.open);

		const openSubmenu = (highlightedEntry: MenuListEntry) => {
			if (highlightedEntry.ref && highlightedEntry.children?.length) {
				highlightedEntry.ref.open = true;
				// The reason we bother taking `highlightdEntry` as an argument is because, when this function is called, it can ensure `highlightedEntry` is not undefined.
				// But here we still have to set `highlighted` to itself so Svelte knows to reactively update it after we set its `.ref.open` property.
				highlighted = highlighted;

				// Highlight first item
				highlightedEntry.ref.setHighlighted(highlightedEntry.children[0][0]);
			}
		};

		if (!menuOpen && (e.key === " " || e.key === "Enter")) {
			// Allow opening menu with space or enter
			open = true;
			highlighted = activeEntry;
		} else if (menuOpen && openChild >= 0) {
			// Redirect the keyboard navigation to a submenu if one is open
			const shouldCloseStack = flatEntries[openChild].ref?.keydown(e, true);

			// Highlight the menu item in the parent list that corresponds with the open submenu
			if (e.key !== "Escape" && highlighted) setHighlighted(flatEntries[openChild]);

			// Handle the child closing the entire menu stack
			if (shouldCloseStack) {
				open = false;
				return true;
			}
		} else if ((menuOpen || interactive) && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
			// Navigate to the next and previous entries with arrow keys

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
		} else if (menuOpen && e.key === "Escape") {
			// Close menu with escape key
			open = false;

			// Reset active to before open
			setHighlighted(activeEntry);
		} else if (menuOpen && highlighted && e.key === "Enter") {
			// Handle clicking on an option if enter is pressed
			if (!highlighted.children?.length) onEntryClick(highlighted);
			else openSubmenu(highlighted);

			// Stop the event from triggering a press on a new dialog
			e.preventDefault();

			// Enter should close the entire menu stack
			return true;
		} else if (menuOpen && highlighted && e.key === "ArrowRight") {
			// Right arrow opens a submenu
			openSubmenu(highlighted);
		} else if (menuOpen && e.key === "ArrowLeft") {
			// Left arrow closes a submenu
			if (submenu) open = false;
		}

		// By default, keep the menu stack open
		return false;
	}

	export function setHighlighted(newHighlight: MenuListEntry | undefined) {
		highlighted = newHighlight;
		// Interactive menus should keep the active entry the same as the highlighted one
		if (interactive && newHighlight?.value !== activeEntry?.value && newHighlight) dispatch("activeEntry", newHighlight);
	}

	export function scrollViewTo(distanceDown: number) {
		scroller?.div()?.scrollTo(0, distanceDown);
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
	<!-- If we put the scrollableY on the layoutcol for non-font dropdowns then for some reason it always creates a tiny scrollbar.
	However when we are using the virtual scrolling then we need the layoutcol to be scrolling so we can bind the events without using `self`. -->
	<LayoutCol
		bind:this={scroller}
		scrollableY={scrollableY && virtualScrollingEntryHeight !== 0}
		on:scroll={onScroll}
		styles={{ "min-width": virtualScrollingEntryHeight ? `${minWidth}px` : `inherit` }}
	>
		{#if virtualScrollingEntryHeight}
			<LayoutRow class="scroll-spacer" styles={{ height: `${virtualScrollingStartIndex * virtualScrollingEntryHeight}px` }} />
		{/if}
		{#each entries as section, sectionIndex (sectionIndex)}
			{#if sectionIndex > 0}
				<Separator type="Section" direction="Vertical" />
			{/if}
			{#each virtualScrollingEntryHeight ? section.slice(virtualScrollingStartIndex, virtualScrollingEndIndex) : section as entry, entryIndex (entryIndex + startIndex)}
				<LayoutRow
					class="row"
					classes={{ open: isEntryOpen(entry), active: entry.label === highlighted?.label, disabled: Boolean(entry.disabled) }}
					styles={{ height: virtualScrollingEntryHeight || "20px" }}
					{tooltip}
					on:click={() => !entry.disabled && onEntryClick(entry)}
					on:pointerenter={() => !entry.disabled && onEntryPointerEnter(entry)}
					on:pointerleave={() => !entry.disabled && onEntryPointerLeave(entry)}
				>
					{#if entry.icon && drawIcon}
						<IconLabel icon={entry.icon} class="entry-icon" />
					{:else if drawIcon}
						<div class="no-icon" />
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
						<div class="no-submenu-arrow" />
					{/if}

					{#if entry.children}
						<!-- TODO: Solve the red underline error on the bind:this below -->
						<svelte:self on:naturalWidth open={entry.ref?.open || false} direction="TopRight" entries={entry.children} {minWidth} {drawIcon} {scrollableY} bind:this={entry.ref} />
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
					margin-left: 4px;
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

					svg {
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
