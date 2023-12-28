<script lang="ts">
	import { createEventDispatcher, getContext, onMount, tick } from "svelte";

	import type { FontsState } from "@graphite/state-providers/fonts";
	import type { MenuListEntry } from "@graphite/wasm-communication/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const fonts = getContext<FontsState>("fonts");

	const dispatch = createEventDispatcher<{
		fontFamily: string;
		fontStyle: string;
		changeFont: { fontFamily: string; fontStyle: string; fontFileUrl: string | undefined };
	}>();

	let menuList: MenuList | undefined;

	export let fontFamily: string;
	export let fontStyle: string;
	export let isStyle = false;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;

	let open = false;
	let entries: MenuListEntry[] = [];
	let activeEntry: MenuListEntry | undefined = undefined;
	let minWidth = isStyle ? 0 : 300;

	$: watchFont(fontFamily, fontStyle);

	async function watchFont(..._: string[]) {
		// We set this function's result to a local variable to avoid reading from `entries` which causes Svelte to trigger an update that results in an infinite loop
		const newEntries = await getEntries();
		entries = newEntries;
		activeEntry = getActiveEntry(newEntries);
	}

	async function setOpen() {
		open = true;

		// Scroll to the active entry (the scroller div does not yet exist so we must wait for the component to render)
		await tick();

		if (activeEntry) {
			const index = entries.indexOf(activeEntry);
			menuList?.scrollViewTo(Math.max(0, index * 20 - 190));
		}
	}

	function toggleOpen() {
		if (!disabled) {
			open = !open;

			if (open) setOpen();
		}
	}

	async function selectFont(newName: string) {
		let family;
		let style;

		if (isStyle) {
			dispatch("fontStyle", newName);

			family = fontFamily;
			style = newName;
		} else {
			dispatch("fontFamily", newName);

			family = newName;
			style = "Normal (400)";
		}

		const fontFileUrl = await fonts.getFontFileUrl(family, style);
		dispatch("changeFont", { fontFamily: family, fontStyle: style, fontFileUrl });
	}

	async function getEntries(): Promise<MenuListEntry[]> {
		const x = isStyle ? fonts.getFontStyles(fontFamily) : fonts.fontNames();
		return (await x).map((entry: { name: string; url: URL | undefined }) => ({
			label: entry.name,
			value: entry.name,
			font: entry.url,
			action: () => selectFont(entry.name),
		}));
	}

	function getActiveEntry(entries: MenuListEntry[]): MenuListEntry {
		const selectedChoice = isStyle ? fontStyle : fontFamily;

		return entries.find((entry) => entry.value === selectedChoice) as MenuListEntry;
	}

	onMount(async () => {
		entries = await getEntries();

		activeEntry = getActiveEntry(entries);
	});
</script>

<!-- TODO: Combine this widget into the DropdownInput widget -->
<LayoutRow class="font-input">
	<LayoutRow
		class="dropdown-box"
		classes={{ disabled }}
		styles={{ "min-width": `${minWidth}px` }}
		{tooltip}
		tabindex={disabled ? -1 : 0}
		on:click={toggleOpen}
		on:keydown={(e) => menuList?.keydown(e, false)}
		data-floating-menu-spawner
	>
		<TextLabel class="dropdown-label">{activeEntry?.value || ""}</TextLabel>
		<IconLabel class="dropdown-arrow" icon="DropdownArrow" />
	</LayoutRow>
	<MenuList
		on:naturalWidth={({ detail }) => isStyle && (minWidth = detail)}
		{activeEntry}
		on:activeEntry={({ detail }) => (activeEntry = detail)}
		{open}
		on:open={({ detail }) => (open = detail)}
		entries={[entries]}
		minWidth={isStyle ? 0 : minWidth}
		virtualScrollingEntryHeight={isStyle ? 0 : 20}
		scrollableY={true}
		bind:this={menuList}
	/>
</LayoutRow>

<style lang="scss" global>
	.font-input {
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
			}

			&.open {
				border-radius: 2px 2px 0 0;
			}

			&.disabled {
				background: var(--color-2-mildblack);

				span {
					color: var(--color-8-uppergray);
				}
			}
		}

		.menu-list .floating-menu-container .floating-menu-content {
			max-height: 400px;
			padding: 4px 0;
		}
	}
</style>
