<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { IconName } from "@graphite/icons";
	import type { MenuListEntry, ActionShortcut } from "@graphite/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import ConditionalWrapper from "@graphite/components/layout/ConditionalWrapper.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const dispatch = createEventDispatcher<{ selectedEntryValuePath: string[] }>();

	let self: MenuList;

	// Note: IconButton should instead be used if only an icon, but no label, is desired.
	// However, if multiple TextButton widgets are used in a group with only some having no label, this component is able to accommodate that.

	// Content
	export let label: string;
	export let icon: IconName | undefined = undefined;
	export let hoverIcon: IconName | undefined = undefined;
	export let disabled = false;
	// Children
	export let menuListChildren: MenuListEntry[][] | undefined = undefined;
	export let menuListChildrenHash: bigint | undefined = undefined;
	// Styling
	export let emphasized = false;
	export let flush = false;
	export let narrow = false;
	// Sizing
	export let minWidth = 0;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	// TODO: Replace this with an event binding (and on other components that do this)
	export let action: (() => void) | undefined;

	$: menuListChildrenExists = (menuListChildren?.length ?? 0) > 0;

	// Handles either a button click or, if applicable, the opening of the menu list floating menu
	function onClick(e: MouseEvent) {
		// If there's no menu to open, trigger the action
		if ((menuListChildren?.length ?? 0) === 0) {
			// Call the action
			if (!disabled) action?.();

			// Exit early so we don't continue on and try to open the menu
			return;
		}

		// Focus the target so that keyboard inputs are sent to the dropdown
		(e.target as HTMLElement | undefined)?.focus();

		// Open the menu list floating menu
		if (self) self.open = true;
		else throw new Error("The menu bar floating menu has no reference to `self`");
	}
</script>

<ConditionalWrapper condition={menuListChildrenExists} wrapperClass="text-button-container">
	<button
		class="text-button"
		class:open={self?.open}
		class:hover-icon={hoverIcon && !disabled}
		class:emphasized
		class:disabled
		class:narrow
		class:flush
		style:min-width={minWidth > 0 ? `${minWidth}px` : undefined}
		data-tooltip-label={tooltipLabel}
		data-tooltip-description={tooltipDescription}
		data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
		data-emphasized={emphasized || undefined}
		data-disabled={disabled || undefined}
		data-text-button
		tabindex={disabled ? -1 : 0}
		data-floating-menu-spawner
		data-block-hover-transfer={menuListChildrenExists ? undefined : ""}
		on:click={onClick}
	>
		{#if icon}
			<IconLabel {icon} />
			{#if hoverIcon && !disabled}
				<IconLabel icon={hoverIcon} />
			{/if}
		{/if}
		{#if label}
			<TextLabel>{label}</TextLabel>
		{/if}
	</button>
	{#if menuListChildrenExists}
		<MenuList
			on:open={({ detail }) => self && (self.open = detail)}
			on:selectedEntryValuePath={({ detail }) => dispatch("selectedEntryValuePath", detail)}
			open={self?.open || false}
			entries={menuListChildren || []}
			entriesHash={menuListChildrenHash || 0n}
			direction="Bottom"
			minWidth={240}
			drawIcon={true}
			bind:this={self}
		/>
	{/if}
</ConditionalWrapper>

<style lang="scss" global>
	.text-button-container {
		display: flex;
		position: relative;
	}

	.text-button {
		display: flex;
		justify-content: center;
		align-items: center;
		flex: 0 0 auto;
		white-space: nowrap;
		height: var(--widget-height);
		margin: 0;
		padding: 0 8px;
		box-sizing: border-box;
		border: none;
		border-radius: 2px;
		background: var(--button-background-color);
		color: var(--button-text-color);
		--button-background-color: var(--color-4-dimgray);
		--button-text-color: var(--color-e-nearwhite);
		--widget-height: 24px;

		&.narrow.narrow {
			--widget-height: 20px;
		}

		&:hover,
		&.open {
			--button-background-color: var(--color-6-lowergray);
			--button-text-color: var(--color-f-white);
		}

		&.hover-icon {
			&:not(:hover) .icon-label:nth-of-type(2) {
				display: none;
			}

			&:hover .icon-label:nth-of-type(1) {
				display: none;
			}
		}

		&.disabled {
			--button-background-color: var(--color-4-dimgray);
			--button-text-color: var(--color-8-uppergray);
		}

		&.emphasized {
			--button-background-color: var(--color-e-nearwhite);
			--button-text-color: var(--color-2-mildblack);

			&:hover,
			&.open {
				--button-background-color: var(--color-f-white);
			}

			&.disabled {
				--button-background-color: var(--color-8-uppergray);
			}
		}

		&.flush {
			--button-background-color: none;
			--button-text-color: var(--color-e-nearwhite);

			&:hover,
			&.open {
				--button-background-color: var(--color-5-dullgray);
			}

			&.disabled {
				--button-text-color: var(--color-8-uppergray);
				--button-background-color: none;
			}
		}

		.icon-label {
			fill: var(--button-text-color);

			+ .text-label {
				margin-left: 8px;
			}
		}

		.text-label {
			overflow: hidden;
		}

		// Custom styling for when multiple TextButton widgets are used next to one another in a row or column
		.widget-span.row > & + .text-button,
		.layout-row > & + .text-button {
			margin-left: 8px;
		}
		.widget-span.column > & + .text-button,
		.layout-column > & + .text-button {
			margin-top: 8px;
		}
	}
</style>
