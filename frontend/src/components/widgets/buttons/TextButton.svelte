<script lang="ts">
	import type { IconName } from "@graphite/utility-functions/icons";
	import type { MenuListEntry } from "@graphite/wasm-communication/messages";

	import MenuList from "@graphite/components/floating-menus/MenuList.svelte";
	import ConditionalWrapper from "@graphite/components/layout/ConditionalWrapper.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	let self: MenuList;

	// Note: IconButton should be used if only an icon, but no label, is desired.
	// However, if multiple TextButton widgets are used in a group with only some having no label, this component is able to accommodate that.
	export let label: string;
	export let icon: IconName | undefined = undefined;
	export let emphasized = false;
	export let flush = false;
	export let minWidth = 0;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	export let menuListChildren: MenuListEntry[][] | undefined = undefined;

	// Callbacks
	// TODO: Replace this with an event binding (and on other components that do this)
	export let action: (() => void) | undefined;

	$: menuListChildrenExists = (menuListChildren?.length ?? 0) > 0;

	// Handles either a button click or, if applicable, the opening of the menu list floating menu
	function onClick(e: MouseEvent) {
		// If there's no menu to open, trigger the action
		if ((menuListChildren?.length ?? 0) === 0) {
			// Call the action
			if (action && !disabled) action();

			// Exit early so we don't continue on and try to open the menu
			return;
		}

		// Focus the target so that keyboard inputs are sent to the dropdown
		(e.target as HTMLElement | undefined)?.focus();

		// Open the menu list floating menu
		if (self) {
			self.open = true;
		} else {
			throw new Error("The menu bar floating menu has no associated ref");
		}
	}
</script>

<ConditionalWrapper condition={menuListChildrenExists} wrapperClass="text-button-container">
	<button
		class="text-button"
		class:open={self?.open}
		class:emphasized
		class:disabled
		class:flush
		style:min-width={minWidth > 0 ? `${minWidth}px` : ""}
		title={tooltip}
		data-emphasized={emphasized || undefined}
		data-disabled={disabled || undefined}
		data-text-button
		tabindex={disabled ? -1 : 0}
		data-floating-menu-spawner={menuListChildrenExists ? "" : "no-hover-transfer"}
		on:click={onClick}
		on:keydown={(e) => self?.keydown(e, false)}
	>
		{#if icon}
			<IconLabel {icon} />
		{/if}
		{#if icon && label}
			<Separator type={flush ? "Unrelated" : "Related"} />
		{/if}
		{#if label}
			<TextLabel>{label}</TextLabel>
		{/if}
	</button>
	{#if menuListChildrenExists}
		<MenuList
			on:open={({ detail }) => self && (self.open = detail)}
			open={self?.open || false}
			entries={menuListChildren || []}
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
		height: 24px;
		margin: 0;
		padding: 0 8px;
		box-sizing: border-box;
		border: none;
		border-radius: 2px;
		background: var(--button-background-color);
		color: var(--button-text-color);
		--button-background-color: var(--color-5-dullgray);
		--button-text-color: var(--color-e-nearwhite);

		&:hover,
		&.open {
			--button-background-color: var(--color-6-lowergray);
			--button-text-color: var(--color-f-white);
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
			background: none;

			&:hover,
			&.open {
				background: var(--color-5-dullgray);
			}
		}

		.widget-span.row > & + .text-button,
		.layout-row > & + .text-button {
			margin-left: 8px;
		}

		.widget-span.column > & + .text-button,
		.layout-column > & + .text-button {
			margin-top: 8px;
		}

		.icon-label {
			fill: var(--button-text-color);
		}

		.text-label {
			overflow: hidden;
		}
	}
</style>
