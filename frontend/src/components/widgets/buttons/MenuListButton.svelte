<script lang="ts">
	import type { MenuListEntry } from "@graphite/wasm-communication/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	export let entry: MenuListEntry;

	let entryRef: MenuList;

	$: (entry.ref = entryRef), entry.ref;

	function clickEntry(e: MouseEvent) {
		// If there's no menu to open, trigger the action but don't try to open its non-existant children
		if ((entry.children?.length ?? 0) === 0) {
			if (entry.action && !entry.disabled) entry.action();
			return;
		}

		// Focus the target so that keyboard inputs are sent to the dropdown
		(e.target as HTMLElement | undefined)?.focus();

		if (entry.ref) {
			entry.ref.open = true;
		} else {
			throw new Error("The menu bar floating menu has no associated ref");
		}
	}
</script>

<div class="menu-list-button">
	<!-- svelte-ignore a11y-no-noninteractive-tabindex -->
	<div
		on:click={(e) => clickEntry(e)}
		on:keydown={(e) => entry.ref?.keydown(e, false)}
		class="entry"
		class:open={entry.ref?.open}
		tabindex="0"
		data-floating-menu-spawner={(entry.children?.length ?? 0) > 0 ? "" : "no-hover-transfer"}
	>
		{#if entry.icon}
			<IconLabel icon={entry.icon} />
		{/if}
		{#if entry.label}
			<TextLabel>{entry.label}</TextLabel>
		{/if}
	</div>
	{#if (entry.children?.length ?? 0) > 0}
		<MenuList
			on:open={({ detail }) => entry.ref && (entry.ref.open = detail)}
			open={entry.ref?.open || false}
			entries={entry.children || []}
			direction="Bottom"
			minWidth={240}
			drawIcon={true}
			bind:this={entryRef}
		/>
	{/if}
</div>

<style lang="scss" global>
	.menu-list-button {
		display: flex;
		position: relative;

		.entry {
			display: flex;
			align-items: center;
			white-space: nowrap;
			background: none;
			padding: 0 8px;
			margin: 0;
			border: 0;
			border-radius: 2px;

			&:hover,
			&.open {
				background: var(--color-5-dullgray);
			}
		}
	}
</style>
