<script lang="ts">
	import type { Snippet } from "svelte";

	import { type IconName, type PopoverButtonStyle } from "@graphite/utility-functions/icons";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import type { MenuDirection } from "@graphite/messages.svelte";

	type Props = {
		style?: PopoverButtonStyle;
		menuDirection?: MenuDirection;
		icon?: IconName | undefined;
		tooltip?: string | undefined;
		disabled?: boolean;
		popoverMinWidth?: number;
		// Callbacks
		action?: (() => void) | undefined;
		children?: Snippet;
	};

	let { style = "DropdownArrow", menuDirection = "Bottom", icon = undefined, tooltip = undefined, disabled = false, popoverMinWidth = 1, action = undefined, children }: Props = $props();

	let open = $state(false);

	function onClick() {
		open = true;
		action?.();
	}
</script>

<LayoutRow class="popover-button" classes={{ "has-icon": icon !== undefined, "direction-top": menuDirection === "Top" }}>
	<IconButton class="dropdown-icon" classes={{ open }} {disabled} onclick={() => onClick()} icon={style || "DropdownArrow"} size={16} {tooltip} data-floating-menu-spawner />
	{#if icon !== undefined}
		<IconLabel class="descriptive-icon" classes={{ open }} {disabled} {icon} {tooltip} />
	{/if}

	<FloatingMenu bind:open minWidth={popoverMinWidth} type="Popover" direction={menuDirection || "Bottom"}>
		{@render children?.()}
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
