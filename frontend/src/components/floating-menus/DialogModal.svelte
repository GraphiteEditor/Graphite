<script lang="ts">
	import { getContext, onMount } from "svelte";

	import FloatingMenu from "~/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "~/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "~/src/components/layout/LayoutRow.svelte";
	import TextButton from "~/src/components/widgets/buttons/TextButton.svelte";
	import IconLabel from "~/src/components/widgets/labels/IconLabel.svelte";
	import WidgetLayout from "~/src/components/widgets/WidgetLayout.svelte";
	import type { DialogState } from "~/src/state-providers/dialog";

	const dialog = getContext<DialogState>("dialog");

	let self: FloatingMenu | undefined;

	export function dismiss() {
		dialog.dismissDialog();
	}

	onMount(() => {
		// Focus the first button in the popup
		const emphasizedOrFirstButton = (self?.div()?.querySelector("[data-emphasized]") || self?.div()?.querySelector("[data-text-button]") || undefined) as HTMLButtonElement | undefined;
		emphasizedOrFirstButton?.focus();
	});
</script>

<FloatingMenu open={true} class="dialog-modal" type="Dialog" direction="Center" bind:this={self} data-dialog-modal>
	<LayoutRow>
		<LayoutCol class="icon-column">
			<!-- `$dialog.icon` class exists to provide special sizing in CSS to specific icons -->
			<IconLabel icon={$dialog.icon} class={$dialog.icon.toLowerCase()} />
		</LayoutCol>
		<LayoutCol class="main-column">
			{#if $dialog.widgets.layout.length > 0}
				<WidgetLayout layout={$dialog.widgets} class="details" />
			{/if}
			{#if ($dialog.jsCallbackBasedButtons?.length || NaN) > 0}
				<LayoutRow class="panic-buttons-row">
					{#each $dialog.jsCallbackBasedButtons || [] as button, index (index)}
						<TextButton action={() => button.callback?.()} {...button.props} />
					{/each}
				</LayoutRow>
			{/if}
		</LayoutCol>
	</LayoutRow>
</FloatingMenu>

<style lang="scss" global>
	.dialog-modal {
		position: absolute;
		pointer-events: none;
		width: 100%;
		height: 100%;

		> .floating-menu-container > .floating-menu-content {
			pointer-events: auto;
			padding: 24px;
		}

		.icon-column {
			margin-right: 24px;

			.icon-label {
				width: 80px;
				height: 80px;

				&.file,
				&.copy {
					width: 60px;

					svg {
						width: 80px;
						height: 80px;
						margin: 0 -10px;
					}
				}
			}
		}

		.main-column {
			margin: -4px 0;

			.details.text-label {
				-webkit-user-select: text; // Required as of Safari 15.0 (Graphite's minimum version) through the latest release
				user-select: text;
				white-space: pre-wrap;
				max-width: 400px;
				height: auto;
			}

			.panic-buttons-row {
				height: 32px;
				align-items: center;
			}
		}
	}
</style>
