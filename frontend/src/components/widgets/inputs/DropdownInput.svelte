<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { MenuListEntry } from "@graphite/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const DASH_ENTRY = { value: "", label: "-" };

	const dispatch = createEventDispatcher<{ selectedIndex: number; hoverInEntry: number; hoverOutEntry: number }>();

	let menuList: MenuList | undefined;
	let self: LayoutRow | undefined;

	export let entries: MenuListEntry[][];
	export let selectedIndex: number | undefined = undefined; // When not provided, a dash is displayed
	export let drawIcon = false;
	export let interactive = true;
	export let disabled = false;
	export let narrow = false;
	export let tooltip: string | undefined = undefined;
	export let minWidth = 0;
	export let maxWidth = 0;

	let activeEntry = makeActiveEntry();
	let activeEntrySkipWatcher = false;
	let initialSelectedIndex: number | undefined = undefined;
	let open = false;

	$: watchSelectedIndex(selectedIndex);
	$: watchEntries(entries);
	$: watchActiveEntry(activeEntry);
	$: watchOpen(open);

	function watchOpen(open: boolean) {
		initialSelectedIndex = open ? selectedIndex : undefined;
	}

	// Called only when `selectedIndex` is changed from outside this component
	function watchSelectedIndex(_?: typeof selectedIndex) {
		activeEntrySkipWatcher = true;
		activeEntry = makeActiveEntry();
	}

	// Called only when `entries` is changed from outside this component
	function watchEntries(_?: typeof entries) {
		activeEntrySkipWatcher = true;
		activeEntry = makeActiveEntry();
	}

	// Called when the `activeEntry` two-way binding on this component's MenuList component is changed, or by the `selectedIndex()` watcher above (but we want to skip that case)
	function watchActiveEntry(activeEntry: MenuListEntry) {
		if (activeEntrySkipWatcher) {
			activeEntrySkipWatcher = false;
		} else if (activeEntry !== DASH_ENTRY) {
			// We need to set to the initial value first to track a right history step, as if we hover in initial selection.
			if (initialSelectedIndex !== undefined) dispatch("hoverInEntry", initialSelectedIndex);
			dispatch("selectedIndex", entries.flat().indexOf(activeEntry));
		}
	}

	function dispatchHoverInEntry(hoveredEntry: MenuListEntry) {
		dispatch("hoverInEntry", entries.flat().indexOf(hoveredEntry));
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
		const blurTarget = (e.target as HTMLDivElement | undefined)?.closest("[data-dropdown-input]") || undefined;
		if (blurTarget !== self?.div?.()) open = false;
	}
</script>

<LayoutRow
	class="dropdown-input"
	classes={{ narrow }}
	styles={{
		...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}),
		...(maxWidth > 0 ? { "max-width": `${maxWidth}px` } : {}),
	}}
	bind:this={self}
	data-dropdown-input
>
	<LayoutRow
		class="dropdown-box"
		classes={{ disabled, open }}
		{tooltip}
		on:click={() => !disabled && (open = true)}
		on:blur={unFocusDropdownBox}
		tabindex={disabled ? -1 : 0}
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
		{drawIcon}
		{interactive}
		direction="Bottom"
		scrollableY={true}
		bind:this={menuList}
	/>
</LayoutRow>

<style lang="scss" global>
	.dropdown-input {
		position: relative;
		--widget-height: 24px;

		&.narrow.narrow {
			--widget-height: 20px;
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
