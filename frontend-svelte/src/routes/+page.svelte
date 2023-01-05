<script lang="ts">
import { editor } from "$lib/stores"
import { onMount } from "svelte"

</script>

<h1>Welcome to SvelteKit</h1>
<p>
  Visit <a href="https://kit.svelte.dev">kit.svelte.dev</a> to read the documentation
</p>
<p>
  Graphite version: {$editor?.graphiteDocumentVersion()}
</p>
