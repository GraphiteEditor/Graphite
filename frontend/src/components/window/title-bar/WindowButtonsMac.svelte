<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	const editor = getContext<Editor>("editor");
</script>

<LayoutRow class="window-buttons-mac">
	<div class="close" on:click={() => editor.handle.appWindowClose()} />
	<div class="minimize" on:click={() => editor.handle.appWindowMinimize()} />
	<div class="zoom" on:click={() => editor.handle.appWindowMaximize()} />
</LayoutRow>

<style lang="scss" global>
	.window-buttons-mac {
		flex: 0 0 auto;
		align-items: center;
		margin: 0 8px;

		div {
			flex: 0 0 auto;
			align-items: center;
			width: 11px;
			height: 11px;
			border-radius: 50%;

			& + div {
				margin-left: 8px;
			}

			&.close {
				background: #ff5a52;
			}

			&.minimize {
				background: #e6c029;
			}

			&.zoom {
				background: #54c22b;
			}
		}
	}
</style>
