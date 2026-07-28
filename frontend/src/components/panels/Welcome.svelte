<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { welcomeScreenButtonsLayout } from "/src/stores/portfolio";
	import { pasteFile } from "/src/utility-functions/files";
	import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");

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
				<WidgetLayout layout={$welcomeScreenButtonsLayout} layoutTarget="WelcomeScreenButtons" />
			</LayoutRow>
		</LayoutCol>
	</LayoutCol>
	<LayoutCol class="bottom-message">
		{#if import.meta.env.MODE === "native"}
			<TextLabel italic={true} disabled={true}>
				You are testing Release Candidate 6 of the 1.0 desktop release. Please regularly check Discord for the next testing build and report issues you encounter.
			</TextLabel>
		{:else if new Date() < new Date(2026, 10, 1)}
			<TextLabel italic={true} disabled={true}>
				May 2026 release — <a href="https://youtube.com/watch?v=U3E-sWo2H_M" target="_blank">What's new? (video)</a>
				— Note: Some nodes are renamed; Some older documents may render differently and require manual fixes.
				<a href="https://57130155.graphite.pages.dev/" target="_blank">Need the old version?</a>
			</TextLabel>
		{/if}
	</LayoutCol>
</LayoutCol>

<style lang="scss">
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
