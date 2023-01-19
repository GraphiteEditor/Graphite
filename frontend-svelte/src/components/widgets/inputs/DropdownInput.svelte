<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { MenuListEntry } from "@/wasm-communication/messages";

	import MenuList from "@/components/floating-menus/MenuList.svelte";
	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import IconLabel from "@/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";

	const DASH_ENTRY = { label: "-" };

	// emits: ["update:selectedIndex"],
	const dispatch = createEventDispatcher<{ selectedIndex: number }>();

	let menuList: MenuList;
	let self: LayoutRow;

	export let entries: MenuListEntry[][];
	export let selectedIndex: number | undefined = undefined; // When not provided, a dash is displayed
	export let drawIcon = false;
	export let interactive = true;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;

	let activeEntry = makeActiveEntry();
	let activeEntrySkipWatcher = false;
	let open = false;
	let minWidth = 0;

	$: selectedIndex, watchSelectedIndex();
	$: watchActiveEntry(activeEntry);

	// Called only when `selectedIndex` is changed from outside this component (with v-model)
	function watchSelectedIndex() {
		activeEntrySkipWatcher = true;
		activeEntry = makeActiveEntry();
	}

	// Called when `activeEntry` is changed by the `v-model` on this component's MenuList component, or by the `selectedIndex()` watcher above (but we want to skip that case)
	function watchActiveEntry(activeEntry: MenuListEntry) {
		if (activeEntrySkipWatcher) {
			activeEntrySkipWatcher = false;
		} else if (activeEntry !== DASH_ENTRY) {
			dispatch("selectedIndex", entries.flat().indexOf(activeEntry));
		}
	}

	function makeActiveEntry(): MenuListEntry {
		const allEntries = entries.flat();

		if (selectedIndex !== undefined && selectedIndex >= 0 && selectedIndex < allEntries.length) {
			return allEntries[selectedIndex];
		}
		return DASH_ENTRY;
	}

	function unFocusDropdownBox(e: FocusEvent) {
		const blurTarget = (e.target as HTMLDivElement | undefined)?.closest("[data-dropdown-input]");
		if (blurTarget !== self.div()) open = false;
	}
</script>

<LayoutRow class="dropdown-input" bind:this={self} data-dropdown-input>
	<LayoutRow
		class="dropdown-box"
		classes={{ disabled, open, "sharp-right-corners": sharpRightCorners }}
		styles={{ minWidth: `${minWidth}px` }}
		{tooltip}
		on:click={() => !disabled && (open = true)}
		on:blur={unFocusDropdownBox}
		on:keydown={(e) => menuList.keydown(e, false)}
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
		{activeEntry}
		on:activeEntry={({ detail }) => (activeEntry = detail)}
		{open}
		on:open={({ detail }) => (open = detail)}
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

		.dropdown-box {
			align-items: center;
			white-space: nowrap;
			background: var(--color-1-nearblack);
			height: 24px;
			border-radius: 2px;

			.dropdown-label {
				margin: 0;
				margin-left: 8px;
				flex: 1 1 100%;
			}

			.dropdown-icon {
				margin: 4px;
				flex: 0 0 auto;

				& + .dropdown-label {
					margin-left: 0;
				}
			}

			.dropdown-arrow {
				margin: 6px 2px;
				flex: 0 0 auto;
			}

			&:hover,
			&.open {
				background: var(--color-6-lowergray);

				span {
					color: var(--color-f-white);
				}

				svg {
					fill: var(--color-f-white);
				}
			}

			&.open {
				border-radius: 2px 2px 0 0;
			}

			&.disabled {
				background: var(--color-2-mildblack);

				span {
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
