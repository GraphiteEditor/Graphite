<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import MenuList from "/src/components/floating-menus/MenuList.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { MenuListEntry, ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	const DASH_ENTRY: MenuListEntry = {
		value: "",
		label: "-",
		icon: undefined,
		disabled: false,
		children: [],
		childrenHash: 0n,
		font: undefined,
		tooltipLabel: "",
		tooltipDescription: "",
		tooltipShortcut: undefined,
	};

	const dispatch = createEventDispatcher<{ selectedIndex: number; hoverInEntry: number; hoverOutEntry: number; fileDrop: File }>();

	let self: LayoutRow | undefined;

	// Content
	export let selectedIndex: number | undefined = undefined; // When not provided, a dash is displayed
	export let drawIcon = false;
	export let disabled = false;
	// Children
	export let entries: MenuListEntry[][];
	export let entriesHash: bigint | undefined = undefined;
	// Styling
	export let narrow = false;
	// Behavior
	export let virtualScrolling = false;
	export let interactive = true;
	export let takesFileDrop = false;
	// Sizing
	export let minWidth = 0;
	export let maxWidth = 0;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	let activeEntry = makeActiveEntry();
	let activeEntrySkipWatcher = false;
	let initialSelectedIndex: number | undefined = undefined;
	let activeEntryOnOpen: string | undefined = undefined;
	let open = false;
	let fileDragOver = false;

	$: watchSelectedIndex(selectedIndex);
	$: watchEntries(entries);
	$: watchActiveEntry(activeEntry);
	$: watchOpen(open);

	function watchOpen(open: boolean) {
		if (open) {
			initialSelectedIndex = selectedIndex;
			activeEntryOnOpen = activeEntry.value;
		} else {
			// Suppress hoverOutEntry if a new selection was made
			const selectionMade = activeEntryOnOpen !== undefined && activeEntry.value !== activeEntryOnOpen;
			if (initialSelectedIndex !== undefined && !selectionMade) {
				dispatch("hoverOutEntry", initialSelectedIndex);
			}
			initialSelectedIndex = undefined;
			activeEntryOnOpen = undefined;
		}
	}

	// Called only when `selectedIndex` is changed from outside this component
	function watchSelectedIndex(_: typeof selectedIndex) {
		activeEntrySkipWatcher = true;
		activeEntry = makeActiveEntry();
	}

	// Called only when `entries` is changed from outside this component
	function watchEntries(_: typeof entries) {
		activeEntrySkipWatcher = true;
		activeEntry = makeActiveEntry();
	}

	// Called when the `activeEntry` two-way binding on this component's MenuList component is changed, or by the `watchSelectedIndex()` watcher above (but we want to skip that case)
	function watchActiveEntry(activeEntry: MenuListEntry) {
		if (activeEntrySkipWatcher) {
			activeEntrySkipWatcher = false;
		} else if (activeEntry !== DASH_ENTRY) {
			if (initialSelectedIndex !== undefined) dispatch("hoverInEntry", initialSelectedIndex);
			const index = entries.flat().findIndex((entry) => entry.value === activeEntry.value);
			if (index !== -1) {
				dispatch("selectedIndex", index);
			} else {
				// eslint-disable-next-line no-console
				console.error("Selected index not found in entries:", activeEntry);
			}
		}
	}

	function dispatchHoverInEntry(hoveredEntry: MenuListEntry) {
		const index = entries.flat().findIndex((entry) => entry.value === hoveredEntry.value);

		if (index !== -1) {
			dispatch("hoverInEntry", index);
		} else {
			// eslint-disable-next-line no-console
			console.error("Hovered entry not found in entries:", hoveredEntry);
		}
	}

	function dispatchHoverOutEntry() {
		if (initialSelectedIndex !== undefined) dispatch("hoverOutEntry", initialSelectedIndex);
	}

	function makeActiveEntry(): MenuListEntry {
		const allEntries = entries.flat();

		if (selectedIndex !== undefined && selectedIndex >= 0 && selectedIndex < allEntries.length) {
			return allEntries[selectedIndex];
		}
		return DASH_ENTRY;
	}

	function unFocusDropdownBox(e: FocusEvent) {
		const blurTarget = (e.target instanceof Element ? e.target.closest("[data-dropdown-input]") : undefined) || undefined;
		if (blurTarget !== self?.div?.()) open = false;
	}

	function takesDraggedFile(e: DragEvent): boolean {
		return takesFileDrop && !disabled && Boolean(e.dataTransfer?.types.includes("Files"));
	}

	function fileDragOverWidget(e: DragEvent) {
		if (!takesDraggedFile(e)) return;

		// The browser refuses the drop unless the dragover is canceled
		e.preventDefault();
		fileDragOver = true;
	}

	function fileDragLeaveWidget(e: DragEvent) {
		// Moving between the widget's own children is not leaving it
		if (e.relatedTarget instanceof Node && self?.div?.()?.contains(e.relatedTarget)) return;

		fileDragOver = false;
	}

	function fileDropOnWidget(e: DragEvent) {
		if (!takesDraggedFile(e)) return;

		// The drop is kept from also reaching a panel that imports dropped files
		e.preventDefault();
		e.stopPropagation();
		fileDragOver = false;

		const file = e.dataTransfer?.files[0];
		if (file) dispatch("fileDrop", file);
	}
</script>

<LayoutRow
	class="dropdown-input"
	classes={{ narrow, "file-drag-over": fileDragOver }}
	styles={{
		...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}),
		...(maxWidth > 0 ? { "max-width": `${maxWidth}px` } : {}),
	}}
	on:dragover={fileDragOverWidget}
	on:dragleave={fileDragLeaveWidget}
	on:drop={fileDropOnWidget}
	bind:this={self}
	data-dropdown-input
>
	<LayoutRow
		class="dropdown-box"
		classes={{ disabled, open }}
		{tooltipLabel}
		{tooltipDescription}
		{tooltipShortcut}
		on:click={() => !disabled && (open = true)}
		on:blur={unFocusDropdownBox}
		tabindex={disabled ? undefined : 0}
		data-floating-menu-spawner
	>
		{#if activeEntry.icon}
			<IconLabel class="dropdown-icon" icon={activeEntry.icon} />
		{/if}
		<TextLabel class="dropdown-label">{activeEntry.label}</TextLabel>
		<IconLabel class="dropdown-arrow" icon="DropdownArrow" />
	</LayoutRow>
	<MenuList
		on:naturalWidth={({ detail }) => (minWidth = detail)}
		on:activeEntry={({ detail }) => (activeEntry = detail)}
		on:hoverInEntry={({ detail }) => dispatchHoverInEntry(detail)}
		on:hoverOutEntry={() => dispatchHoverOutEntry()}
		on:open={({ detail }) => (open = detail)}
		{open}
		{activeEntry}
		{entries}
		entriesHash={entriesHash || 0n}
		{drawIcon}
		{interactive}
		{virtualScrolling}
		direction="Bottom"
		scrollableY={true}
	/>
</LayoutRow>

<style lang="scss">
	.dropdown-input {
		position: relative;
		--widget-height: 24px;

		&.narrow.narrow {
			--widget-height: 20px;
		}

		&.file-drag-over::after {
			content: "";
			position: absolute;
			inset: -4px;
			border: 1px dashed var(--color-e-nearwhite);
			border-radius: 4px;
			pointer-events: none;
		}

		.dropdown-box {
			align-items: center;
			white-space: nowrap;
			border-radius: 2px;
			background: var(--color-1-nearblack);
			height: var(--widget-height);

			.dropdown-label {
				margin: 0;
				margin-left: 8px;
				flex: 1 1 100%;
			}

			.dropdown-icon {
				margin: 4px 8px;
				flex: 0 0 auto;

				& + .dropdown-label {
					margin-left: 0;
				}
			}

			.dropdown-arrow {
				margin: 4px;
				margin-right: 2px;
				flex: 0 0 auto;
			}

			&:hover,
			&.open {
				background: var(--color-4-dimgray);
			}

			&.disabled {
				background: var(--color-2-mildblack);

				.text-label {
					color: var(--color-8-uppergray);
				}

				svg {
					fill: var(--color-8-uppergray);
				}
			}
		}

		.menu-list .floating-menu-container .floating-menu-content {
			max-height: 400px;
		}
	}
</style>
