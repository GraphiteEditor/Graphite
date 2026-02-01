<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { Layout } from "@graphite/messages";
	import { patchLayout, UpdateWelcomeScreenButtonsLayout } from "@graphite/messages";
	import { pasteFile } from "@graphite/utility-functions/files";
	import { isDesktop } from "@graphite/utility-functions/platform";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const editor = getContext<Editor>("editor");

	let welcomePanelButtonsLayout: Layout = [];

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateWelcomeScreenButtonsLayout, (data) => {
			patchLayout(welcomePanelButtonsLayout, data);
			welcomePanelButtonsLayout = welcomePanelButtonsLayout;
		});
	});

	onDestroy(() => {
		editor.subscriptions.unsubscribeJsMessage(UpdateWelcomeScreenButtonsLayout);
	});

	function dropFile(e: DragEvent) {
		if (!e.dataTransfer) return;

		e.preventDefault();

		Array.from(e.dataTransfer.items).forEach(async (item) => await pasteFile(item, editor));
	}
</script>

<LayoutCol class="welcome-panel" on:dragover={(e) => e.preventDefault()} on:drop={dropFile}>
	<LayoutCol class="top-spacer"></LayoutCol>
	<LayoutCol class="content-container">
		<LayoutCol class="content">
			<LayoutRow class="logotype">
				<IconLabel icon="GraphiteLogotypeSolid" />
			</LayoutRow>
			<LayoutRow class="actions">
				<WidgetLayout layout={welcomePanelButtonsLayout} layoutTarget="WelcomeScreenButtons" />
			</LayoutRow>
		</LayoutCol>
	</LayoutCol>
	<LayoutCol class="bottom-message">
		<TextLabel italic={true} disabled={true}>
			{#if isDesktop()}
				You are testing Release Candidate 3 of the 1.0 desktop release. Please regularly check Discord for the next testing build and report issues you encounter.
			{/if}
		</TextLabel>
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.welcome-panel {
		background: var(--color-2-mildblack);
		margin: 4px;
		border-radius: 2px;
		justify-content: space-between;

		.content-container {
			flex: 0 0 auto;
			justify-content: center;

			.content {
				flex: 0 0 auto;
				align-items: center;

				.logotype {
					margin-top: 8px;
					margin-bottom: 40px;

					svg {
						width: auto;
						height: 120px;
					}
				}

				.actions {
					margin-bottom: 8px;

					table {
						border-spacing: 8px;
						margin: -8px;

						td {
							padding: 0;
						}
					}
				}
			}
		}

		.top-spacer {
			flex: 0 1 48px;
		}

		.bottom-message {
			flex: 0 0 48px;
			align-items: center;
			justify-content: end;

			.text-label {
				white-space: wrap;
				margin: 0 1em;
			}
		}
	}
</style>
