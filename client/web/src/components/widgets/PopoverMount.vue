<template>
	<div class="popover-mount" v-if="open">
		<div class="tail left"></div>
		<div class="popover">
			<div class="popover-content" ref="popoverContent">
				<slot></slot>
			</div>
		</div>
	</div>
</template>

<style lang="scss">
.popover-mount {
	position: absolute;
	width: 0;
	height: 0;
	display: flex;
}

.tail {
	width: 0;
	height: 0;
	border-style: solid;
	z-index: 1;

	&.top {
		border-width: 0 6px 8px 6px;
		border-color: transparent transparent #222222e6 transparent;
		margin-left: -6px;
		margin-top: 2px;
	}

	&.bottom {
		border-width: 8px 6px 0 6px;
		border-color: #222222e6 transparent transparent transparent;
		margin-left: -6px;
		margin-bottom: 2px;
	}

	&.left {
		border-width: 6px 8px 6px 0;
		border-color: transparent #222222e6 transparent transparent;
		margin-top: -6px;
		margin-left: 2px;
	}

	&.right {
		border-width: 6px 0 6px 8px;
		border-color: transparent transparent transparent #222222e6;
		margin-top: -6px;
		margin-right: 2px;
	}
}

.popover {
	display: flex;
	align-items: center;

	.popover-content {
		background: #222222e6;
		box-shadow: #000 0 0 4px;
		border-radius: 4px;
		color: #eee;
		font-size: inherit;
		padding: 8px;
		z-index: 0;
		display: flex;
		// This `position: relative` is used to allow `top`/`right`/`bottom`/`left` properties to shift the content back from overflowing the workspace
		position: relative;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		open: { type: Boolean, default: false },
	},
	updated() {
		const popoverContent = this.$refs.popoverContent as HTMLElement;
		const workspace = document.querySelector(".workspace");
		if (popoverContent && workspace) {
			const workspaceBounds = workspace.getBoundingClientRect();

			const popoverBounds = popoverContent.getBoundingClientRect();

			const bottomOffset = workspaceBounds.bottom - popoverBounds.bottom - 8;
			if (bottomOffset < 0) popoverContent.style.top = `${bottomOffset}px`;
		}
	},
});
</script>
