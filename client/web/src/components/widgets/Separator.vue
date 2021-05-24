<template>
	<div class="separator" :class="[direction.toLowerCase(), type.toLowerCase()]">
		<div v-if="type === SeparatorType.Section"></div>
	</div>
</template>

<style lang="scss">
.separator {
	&.vertical {
		&.related {
			margin-top: 4px;
		}

		&.unrelated {
			margin-top: 8px;
		}

		&.section {
			width: 100%;
			margin: 8px 0;

			div {
				height: 1px;
				width: calc(100% - 8px);
				margin: 0 4px;
				background: var(--color-7-middlegray);
			}
		}
	}

	&.horizontal {
		&.related {
			margin-left: 4px;
		}

		&.unrelated {
			margin-left: 8px;
		}

		&.section {
			height: 100%;
			margin: 0 8px;

			div {
				height: calc(100% - 8px);
				width: 1px;
				margin: 4px 0;
				background: var(--color-7-middlegray);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export enum SeparatorDirection {
	"Horizontal" = "Horizontal",
	"Vertical" = "Vertical",
}

export enum SeparatorType {
	"Related" = "Related",
	"Unrelated" = "Unrelated",
	"Section" = "Section",
}

export default defineComponent({
	components: {},
	props: {
		direction: { type: String, default: SeparatorDirection.Horizontal },
		type: { type: String, default: SeparatorType.Unrelated },
	},
	data() {
		return {
			SeparatorDirection,
			SeparatorType,
		};
	},
});
</script>
