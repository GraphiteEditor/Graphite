<script lang="ts">
	import { type IconName, type PopoverButtonStyle } from "@graphite/utility-functions/icons";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	export let style: PopoverButtonStyle = "DropdownArrow";
	export let icon: IconName | undefined = undefined;
	export let tooltip: string | undefined = undefined;
	export let disabled = false;
	export let popoverMinWidth = 1;

	// Callbacks
	export let action: (() => void) | undefined = undefined;

	let open = false;

	function onClick() {
		open = true;
		action?.();
	}
</script>

<LayoutRow class="popover-button" classes={{ "has-icon": icon !== undefined }}>
	<IconButton class="dropdown-icon" classes={{ open }} {disabled} action={() => onClick()} icon={style || "DropdownArrow"} size={16} {tooltip} data-floating-menu-spawner />
	{#if icon !== undefined}
		<IconLabel class="descriptive-icon" classes={{ open }} {disabled} {icon} {tooltip} />
	{/if}

	<FloatingMenu {open} on:open={({ detail }) => (open = detail)} minWidth={popoverMinWidth} type="Popover" direction="Bottom">
		<slot />
	</FloatingMenu>
</LayoutRow>

<style lang="scss" global>
	.popover-button {
		position: relative;
		width: 16px;
		height: 24px;
		flex: 0 0 auto;

		&.has-icon {
			width: 36px;

			.dropdown-icon {
				padding-left: calc(36px - 16px);
				box-sizing: content-box;
			}
		}

		.dropdown-icon {
			width: 16px;
			height: 100%;
			padding: 0;
			border: none;
			border-radius: 2px;
			fill: var(--color-e-nearwhite);

			&:hover:not(.disabled),
			&.open:not(.disabled) {
				background: var(--color-5-dullgray);
			}
		}

		.descriptive-icon {
			width: 16px;
			height: 16px;
			margin: auto 0;
			margin-left: calc(-16px - 16px);
			pointer-events: none;
		}

		.floating-menu {
			left: 50%;
			bottom: 0;
		}
	}
</style>
