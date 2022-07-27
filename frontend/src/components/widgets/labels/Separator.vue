<template>
	<div class="separator" :class="[direction.toLowerCase(), type.toLowerCase()]">
		<div v-if="['Section', 'List'].includes(type)"></div>
	</div>
</template>

<style lang="scss">
.separator {
	&.vertical {
		flex: 0 0 auto;

		&.related {
			height: 4px;
		}

		&.unrelated {
			height: 8px;
		}

		&.section,
		&.list {
			width: 100%;

			div {
				height: 1px;
				width: calc(100% - 8px);
				margin: 0 4px;
				background: var(--color-7-middlegray);
			}
		}

		&.section {
			margin: 8px 0;
		}

		&.list {
			margin: 4px 0;
		}
	}

	&.horizontal {
		flex: 0 0 auto;

		&.related {
			width: 4px;
		}

		&.unrelated {
			width: 8px;
		}

		&.section,
		&.list {
			height: 100%;

			div {
				height: calc(100% - 8px);
				width: 1px;
				margin: 4px 0;
				background: var(--color-7-middlegray);
			}
		}

		&.section {
			margin: 0 8px;
		}

		&.list {
			margin: 0 4px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { SeparatorDirection, SeparatorType } from "@/wasm-communication/messages";

export default defineComponent({
	props: {
		direction: { type: String as PropType<SeparatorDirection>, default: "Horizontal" },
		type: { type: String as PropType<SeparatorType>, default: "Unrelated" },
	},
});
</script>
