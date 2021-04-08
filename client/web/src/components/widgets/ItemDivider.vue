<template>
	<div class="item-spacer" :class="{ horizontal: horizontal }">
		<div></div>
	</div>
</template>

<style lang="scss">
.item-spacer {
	margin-left: 0;

	& + * {
		margin-left: 0;
	}

	&:not(.horizontal) {
		height: 100%;
		padding: 0 8px;

		div {
			height: calc(100% - 8px);
			width: 1px;
			margin: 4px 0;
			background: #888;
		}
	}

	&.horizontal {
		width: 100%;
		padding: 8px 0;

		div {
			height: 1px;
			width: calc(100% - 8px);
			margin: 0 4px;
			background: #888;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {
	},
	props: {
		horizontal: { type: Boolean, default: false },
	},
});
</script>
