<template>
	<div style="display: flex">
		<div tabindex="0" style="width: 50%; outline: none">
			<App />
		</div>
		<div tabindex="0" style="width: 50%; outline: none">
			<App />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";

import App from "@/App.vue";

export default defineComponent({
	components: { App },
});
</script>
