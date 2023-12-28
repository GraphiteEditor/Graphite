<script lang="ts">
	import { getContext, onMount } from "svelte";

	import { githubUrl } from "@graphite/io-managers/panic";
	import { wipeDocuments } from "@graphite/io-managers/persistence";

	import type { DialogState } from "@graphite/state-providers/dialog";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const dialog = getContext<DialogState>("dialog");

	let self: FloatingMenu | undefined;

	onMount(() => {
		// Focus the button which is marked as emphasized, or otherwise the first button, in the popup
		const emphasizedOrFirstButton = (self?.div()?.querySelector("[data-emphasized]") || self?.div()?.querySelector("[data-text-button]") || undefined) as HTMLButtonElement | undefined;
		emphasizedOrFirstButton?.focus();
	});
</script>

<!-- TODO: Use https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dialog for improved accessibility -->
<FloatingMenu open={true} class="dialog" type="Dialog" direction="Center" bind:this={self} data-dialog>
	<LayoutRow class="header-area">
		<!-- `$dialog.icon` class exists to provide special sizing in CSS to specific icons -->
		<IconLabel icon={$dialog.icon} class={$dialog.icon.toLowerCase()} />
		<TextLabel>{$dialog.title}</TextLabel>
	</LayoutRow>
	<LayoutRow class="content">
		<LayoutCol class="column-1">
			{#if $dialog.column1.layout.length > 0}
				<WidgetLayout layout={$dialog.column1} class="details" />
			{/if}
			{#if $dialog.panicDetails}
				<div class="widget-layout details">
					<div class="widget-span row"><TextLabel bold={true}>The editor crashed — sorry about that</TextLabel></div>
					<div class="widget-span row"><TextLabel>Please report this by filing an issue on GitHub:</TextLabel></div>
					<div class="widget-span row"><TextButton label="Report Bug" icon="Warning" flush={true} action={() => window.open(githubUrl($dialog.panicDetails), "_blank")} /></div>
					<div class="widget-span row"><TextLabel multiline={true}>Reload the editor to continue. If this occurs<br />immediately on repeated reloads, clear storage:</TextLabel></div>
					<div class="widget-span row">
						<TextButton
							label="Clear Saved Documents"
							icon="Trash"
							flush={true}
							action={async () => {
								await wipeDocuments();
								window.location.reload();
							}}
						/>
					</div>
				</div>
			{/if}
		</LayoutCol>
		{#if $dialog.column2.layout.length > 0}
			<LayoutCol class="column-2">
				<WidgetLayout layout={$dialog.column2} class="details" />
			</LayoutCol>
		{/if}
	</LayoutRow>
	<LayoutRow class="footer-area">
		{#if $dialog.buttons.layout.length > 0}
			<WidgetLayout layout={$dialog.buttons} class="details" />
		{/if}
		{#if $dialog.panicDetails}
			<TextButton label="Copy Error Log" action={() => navigator.clipboard.writeText($dialog.panicDetails)} />
			<TextButton label="Reload" emphasized={true} action={() => window.location.reload()} />
		{/if}
	</LayoutRow>
</FloatingMenu>

<style lang="scss" global>
	.dialog {
		position: absolute;
		pointer-events: none;
		width: 100%;
		height: 100%;

		> .floating-menu-container > .floating-menu-content {
			pointer-events: auto;
			padding: 0;
		}

		.header-area,
		.footer-area {
			background: var(--color-1-nearblack);
		}

		.header-area,
		.footer-area,
		.content {
			padding: 16px 24px;
		}

		.header-area {
			border-radius: 4px 4px 0 0;

			.icon-label {
				width: 24px;
				height: 24px;
			}

			.text-label {
				margin-left: 12px;
				line-height: 24px;
			}
		}

		.content {
			margin: -4px 0;

			.column-1 + .column-2 {
				margin-left: 48px;

				.text-button {
					justify-content: left;
				}
			}

			.details.text-label {
				-webkit-user-select: text; // Required as of Safari 15.0 (Graphite's minimum version) through the latest release
				user-select: text;
				white-space: pre-wrap;
				max-width: 400px;
				height: auto;
			}

			.radio-input button {
				flex-grow: 1;
			}

			// Used by the "Open Demo Artwork" dialog
			.image-label {
				border-radius: 2px;
			}
		}

		.footer-area {
			border-radius: 0 0 4px 4px;
			justify-content: right;

			.text-button {
				min-width: 96px;
			}
		}
	}
</style>
