<template>
	<div class="menu-bar-input">
		<div class="entry"><Icon :icon="'GraphiteLogo'" /></div>
		<div class="entry"><span>File</span></div>
		<div class="entry"><span>Edit</span></div>
		<div class="entry"><span>Document</span></div>
		<div class="entry"><span>View</span></div>
		<div class="entry"><span>Help</span></div>
	</div>
</template>

<style lang="scss">
.menu-bar-input {
	display: flex;

	.entry {
		display: flex;
		align-items: center;
		white-space: nowrap;
		padding: 0 8px;

		svg {
			fill: var(--color-e-nearwhite);
		}

		&:hover {
			background: var(--color-6-lowergray);

			svg {
				fill: var(--color-f-white);
			}

			span {
				color: var(--color-f-white);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Icon from "../labels/Icon.vue";
import { ApplicationPlatform } from "../../window/MainWindow.vue";

export default defineComponent({
	components: { Icon },
	data() {
		return {
			ApplicationPlatform,
		};
	},
});
</script>
