+++
title = "Codebase overview"
template = "book.html"
page_template = "book.html"

[extra]
order = 2 # Chapter number
js = ["video-embed.js"]
+++

<div class="video-embed aspect-16x9">
	<img data-video-embed="vUzIeg8frh4" src="https://static.graphite.rs/content/volunteer/guide/workshop-intro-to-coding-for-graphite-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Workshop: Intro to Coding for Graphite" />
</div>

The Graphite editor is built as a web app powered by Svelte and TypeScript in the frontend and Rust in the backend which is compiled to WebAssembly and run in the browser. The editor makes calls into Graphene, the node graph engine which manages and renders the documents.

The Editor's frontend web code lives in `/frontend/src`. The backend Rust code is located in `/editor`. Graphene is found in `/node-graph`.
