<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { patchLayout } from "/src/utility-functions/widgets";
	import type { Layout } from "/wrapper/pkg/graphite_wasm_wrapper";

	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");

	let statusBarHintsLayout: Layout = [];
	let statusBarInfoLayout: Layout = [];

	onMount(() => {
		subscriptions.subscribeLayoutUpdate("StatusBarHints", (data) => {
			patchLayout(statusBarHintsLayout, data);
			statusBarHintsLayout = statusBarHintsLayout;
		});

		subscriptions.subscribeLayoutUpdate("StatusBarInfo", (data) => {
			patchLayout(statusBarInfoLayout, data);
			statusBarInfoLayout = statusBarInfoLayout;
		});
	});

	onDestroy(() => {
		subscriptions.unsubscribeLayoutUpdate("StatusBarHints");
		subscriptions.unsubscribeLayoutUpdate("StatusBarInfo");
	});
</script>

<LayoutRow class="status-bar">
	<WidgetLayout class="hints" layout={statusBarHintsLayout} layoutTarget="StatusBarHints" />
	<Separator />
	<WidgetLayout class="info" layout={statusBarInfoLayout} layoutTarget="StatusBarInfo" />
</LayoutRow>

<style lang="scss" global>
	.status-bar {
		height: 24px;
		width: 100%;
		flex: 0 0 auto;
		justify-content: space-between;

		.hints {
			overflow: hidden;
			--row-height: 24px;
			margin: 0 4px;
			max-width: calc(100% - 2 * 4px);
			flex: 1 1 auto;

			> * {
				flex: 0 0 auto;
			}

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

		.hints + .separator {
			position: relative;

			&::before {
				content: "";
				position: absolute;
				top: 0;
				bottom: 0;
				left: -40px;
				width: 40px;
				background: linear-gradient(to right, rgba(var(--color-2-mildblack-rgb), 0) 0%, rgba(var(--color-2-mildblack-rgb), 1) 100%);
			}
		}

		.info {
			margin: 0 4px;
			--row-height: 24px;
			justify-content: flex-end;

			.text-label {
				align-items: center;
				flex-shrink: 0;

				+ .text-label {
					margin-left: 4px;
				}
			}
		}
	}
</style>
