<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { Layout } from "@graphite/messages";
	import { patchLayout, UpdateStatusBarHintsLayout } from "@graphite/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const editor = getContext<Editor>("editor");

	let statusBarHintsLayout: Layout = [];

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateStatusBarHintsLayout, (updateStatusBarHintsLayout) => {
			patchLayout(statusBarHintsLayout, updateStatusBarHintsLayout);
			statusBarHintsLayout = statusBarHintsLayout;
		});
	});
</script>

<LayoutRow class="status-bar">
	<WidgetLayout class="hints" layout={statusBarHintsLayout} layoutTarget="StatusBarHints" />
</LayoutRow>

<style lang="scss" global>
	.status-bar {
		height: 24px;
		width: 100%;
		flex: 0 0 auto;

		.hints {
			overflow: hidden;
			--row-height: 24px;
			margin: 0 4px;
			max-width: calc(100% - 2 * 4px);

			.text-label,
			.shortcut-label {
				align-items: center;
				flex-shrink: 0;

				+ .text-label,
				+ .shortcut-label {
					margin-left: 4px;
				}
			}

			.text-label:not(.bold) + .shortcut-label {
				margin-left: 12px;
			}

			.text-label.bold {
				padding: 0 4px;
			}
		}
	}
</style>
