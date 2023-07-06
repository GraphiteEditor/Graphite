<script lang="ts">
	import type { IconName } from "@graphite/utility-functions/icons";
	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import { WidgetLayout } from "~src/wasm-communication/messages";

	export let icon: IconName = "DropdownArrow";
	export let tooltip: string | undefined = undefined;
	export let disabled = false;
	export let optionsWidget: WidgetLayout;
	export let layoutTarget: unknown;

	// Callbacks
	export let action: (() => void) | undefined = undefined;

	let open = false;

	function onClick() {
		open = true;
		action?.();
	}
</script>

<LayoutRow class="popover-button">
	{#if optionsWidget === undefined}
		<IconButton classes={{ open }} {disabled} action={() => onClick()} icon={icon || "DropdownArrow"} size={16} {tooltip} data-floating-menu-spawner />
	{:else}
		<IconButton classes={{ open }} {disabled} action={() => onClick()} icon={icon || "DropdownArrow"} size={16} {optionsWidget} {layoutTarget} data-floating-menu-spawner />
	{/if}

	<FloatingMenu {open} on:open={({ detail }) => (open = detail)} type="Popover" direction="Bottom">
		<slot />
	</FloatingMenu>
</LayoutRow>

<style lang="scss" global>
	.popover-button {
		position: relative;
		width: 16px;
		height: 24px;
		flex: 0 0 auto;

		.floating-menu {
			left: 50%;
			bottom: 0;
		}

		.icon-button.icon-button {
			width: 100%;
			height: 100%;
			padding: 0;
			border: none;
			border-radius: 2px;
			background: var(--color-1-nearblack);
			fill: var(--color-e-nearwhite);

			&:hover,
			&.open {
				background: var(--color-5-dullgray);
			}

			&.disabled {
				background: var(--color-2-mildblack);
				fill: var(--color-8-uppergray);
			}
		}

		// TODO: Refactor this and other complicated cases dealing with joined widget margins and border-radius by adding a single standard set of classes: joined-first, joined-inner, and joined-last
		div[class*="-input"] + & {
			margin-left: 1px;

			.icon-button {
				border-radius: 0 2px 2px 0;
			}
		}
	}
</style>
