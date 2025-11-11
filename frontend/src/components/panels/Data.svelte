<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { defaultWidgetLayout, patchWidgetLayout, UpdateDataPanelLayout } from "@graphite/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const editor = getContext<Editor>("editor");

	let dataPanelLayout = defaultWidgetLayout();

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdateDataPanelLayout, (updateDataPanelLayout) => {
			patchWidgetLayout(dataPanelLayout, updateDataPanelLayout);
			dataPanelLayout = dataPanelLayout;
		});
	});

	onDestroy(() => {
		editor.subscriptions.unsubscribeJsMessage(UpdateDataPanelLayout);
	});
</script>

<LayoutCol class="data-panel">
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={dataPanelLayout} />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.data-panel {
		flex-grow: 1;
		padding: 4px;

		table {
			margin: -4px;
			width: calc(100% + 2 * 4px);

			.text-label {
				white-space: wrap;
			}

			&:not(:first-child) {
				margin-top: 0;
			}

			tr:first-child:has(td:first-child label:empty) ~ tr td:first-child {
				width: 0;
			}
		}

		.widget-span:has(.text-area-input) {
			flex: 1 1 100%;

			.text-area-input textarea {
				height: 100%;
				margin-top: 0;
				margin-bottom: 0;
				resize: none;
			}
		}
	}
</style>
