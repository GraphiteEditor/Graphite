<script lang="ts">
	import { type IconName, type PopoverButtonStyle } from "@graphite/icons";

	import type { MenuDirection, ActionShortcut, Layout, LayoutTarget } from "@graphite/messages";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	export let layoutTarget: LayoutTarget;

	// Content
	export let style: PopoverButtonStyle = "DropdownArrow";
	export let icon: IconName | undefined = undefined;
	export let disabled = false;
	// Children
	export let popoverLayout: Layout;
	export let popoverMinWidth = 1;
	export let menuDirection: MenuDirection = "Bottom";
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (() => void) | undefined = undefined;

	let open = false;

	function onClick() {
		open = true;
		action?.();
	}
</script>

<LayoutRow class="popover-button" classes={{ "has-icon": icon !== undefined, "direction-top": menuDirection === "Top" }}>
	<IconButton
		class="dropdown-icon"
		classes={{ open }}
		{disabled}
		action={() => onClick()}
		icon={style || "DropdownArrow"}
		size={16}
		{tooltipLabel}
		{tooltipDescription}
		{tooltipShortcut}
		data-floating-menu-spawner
	/>
	{#if icon !== undefined}
		<IconLabel class="descriptive-icon" classes={{ open }} {disabled} {icon} {tooltipLabel} {tooltipDescription} {tooltipShortcut} />
	{/if}

	<FloatingMenu {open} on:open={({ detail }) => (open = detail)} minWidth={popoverMinWidth} type="Popover" direction={menuDirection || "Bottom"}>
		<WidgetLayout layout={popoverLayout} {layoutTarget} />
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

			&.direction-top .dropdown-icon .icon-label {
				transform: rotate(180deg);
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

			.floating-menu-content > :first-child:not(:has(:not(.text-label))),
			.floating-menu-content > :first-child:not(:has(:not(.checkbox-input))) {
				margin-top: -8px;
			}

			.floating-menu-content > :last-child:not(:has(:not(.text-label))),
			.floating-menu-content > :last-child:not(:has(:not(.checkbox-input))) {
				margin-bottom: -8px;
			}
		}

		&.direction-top .floating-menu {
			bottom: 100%;
		}
	}
</style>
