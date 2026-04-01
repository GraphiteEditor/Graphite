<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { patchLayout } from "/src/utility-functions/widgets";
	import type { Layout } from "/wrapper/pkg/graphite_wasm_wrapper";

	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");

	let dataPanelLayout: Layout = [];

	onMount(() => {
		subscriptions.subscribeLayoutUpdate("DataPanel", (data) => {
			patchLayout(dataPanelLayout, data);
			dataPanelLayout = dataPanelLayout;
		});
	});

	onDestroy(() => {
		subscriptions.unsubscribeLayoutUpdate("DataPanel");
	});
</script>

<LayoutCol class="data-panel">
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={dataPanelLayout} layoutTarget="DataPanel" />
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
