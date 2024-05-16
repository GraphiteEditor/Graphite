+++
title = "About Graphite"

[extra]
css = ["about.css"]
+++

<section>
<div class="block">

# About Graphite

Graphite is a community-built free software project. If you find it valuable, consider [donating](/donate) or [getting involved](/volunteer) to keep it sustainable.

</div>
</section>

<section>
<div class="block">

## Project

The idea for Graphite began with a desire to create artwork and edit photos using free software that felt user-friendly and truly modern. Over time, that dream evolved to reconsider what "modern" meant for the landscape of 2D graphics editing. By borrowing concepts popular in 3D software, what could a procedural, nondestructive design tool look like if nothing was too ambitious? Answering that question took years of design exploration, leading to a community of savvy developers volunteering to turn that formidable dream into a reality.

</div>
</section>

<section>

<div class="diptych">

<div class="block">

## Mission

Graphite strives to unshackle the creativity of every budding artist and seasoned professional by building the best comprehensive art and design tool that's accessible to all.

Mission success will come when Graphite is an industry standard. A cohesive product vision and focus on innovation over imitation is the strategy that will make that possible.

</div>
<div class="block">

## Funding

Graphite has no investors to answer to and will always be free and open source. [Keavon](#keavon), the project founder, has been personally funding all expenses out-of-pocket and turning down a tech industry salary to work full-time bringing Graphite to life. He asks for [your support](/donate) bearing a small fraction of that cost.

Long-term, donations will be supplemented by revenue from cloud storage/sync and multi-GPU render acceleration services.

</div>

</div>

</section>

<!-- ## Statistics

- [GitHub stars](https://github.com/GraphiteEditor/Graphite/stargazers): <span class="loading-data" data-github-stars></span>
- [Contributors](https://github.com/GraphiteEditor/Graphite/graphs/contributors): <span class="loading-data" data-contributors></span>
- [Code commits](https://github.com/GraphiteEditor/Graphite/commits/master): <span class="loading-data" data-code-commits></span>
- [First line of code](https://github.com/GraphiteEditor/Graphite/commit/bca97cbeff8e38b426cfb410159cb21132062fba): Feb. 14, 2021

<script>
(async () => {
	const response = await fetch("https://api.github.com/repos/graphiteeditor/graphite?per_page=1");
	const json = await response.json();
	const stars = parseInt(json.stargazers_count);
	if (!stars) return;

	document.querySelector("[data-github-stars]").innerText = `${Math.round(stars / 100) / 10}k ⭐`;
})();
(async () => {
	const response = await fetch("https://api.github.com/repos/graphiteeditor/graphite/contributors?per_page=1");
	const link = [...response.headers].find(([header, _]) => header === "link")[1];
	if (!link) return;
	// With one page per contributor, the last past number is the contributor count
	const contributors = parseInt(link.match(/page=(\d+)>; rel="last"/)[1]);
	if (!contributors) return;

	document.querySelector("[data-contributors]").innerText = contributors;
})();
(async () => {
	const response = await fetch("https://api.github.com/repos/graphiteeditor/graphite/commits?per_page=1");
	const link = [...response.headers].find(([header, _]) => header === "link")[1];
	if (!link) return;
	// With one page per commit, the last past number is the commit count
	const commits = parseInt(link.match(/page=(\d+)>; rel="last"/)[1]);
	if (!commits) return;

	document.querySelector("[data-code-commits]").innerText = commits;
})();
</script> -->

<!-- <section id="opener-message">
<div class="block">

## A 2D creative tool made for everyone

With great power comes great accessibility. Graphite is built on the belief that the best creative tools can be powerful and within reach of all, from students to studios.

Graphite is designed with a friendly and intuitive interface where a delightful user experience is of first-class importance. It is available for free under an open source [license](/license) and usable [instantly through a web browser](https://editor.graphite.rs) or an upcoming native client on Windows, Mac, and Linux.

It's easy to learn and teach, yet Graphite's accessible design does not sacrifice versatility for simplicity. The node-based workflow opens doors to an ecosystem of powerful capabilities catering to casual and professional users alike.

</div>
<div class="graphic">
	<img src="https://static.graphite.rs/content/index/brush__2.svg" alt="" />
</div>
</section> -->

<section id="core-team" class="feature-box-outer">
<div class="feature-box-inner">

<h1 class="feature-box-header">Meet the core team</h1>

---

<div class="triptych">

<div class="block" id="keavon">

<img src="https://static.graphite.rs/content/about/core-team-photo-keavon-chambers.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Keavon Chambers" />

## Keavon Chambers

*@Keavon* <span class="flag" title="American">🇺🇸</span>

*Founder, UI & product design, frontend engineering*

Keavon is a creative generalist with a love for the fusion of arts and technology. UX and graphic designer, photographer, game developer, technical artist, and everything in between— he is equal parts designer and engineer. His multidisciplinary background in the digital arts is aptly suited for concocting the unique vision needed to bring Graphite to fruition.

</div>
<div class="block" id="dennis">

<img src="https://static.graphite.rs/content/about/core-team-photo-dennis-kobert.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Dennis Kobert" />

## Dennis Kobert

*@TrueDoctor* <span class="flag" title="German">🇩🇪</span>

*Graphene node engine, research, architecture*

Dennis is a mix between a mathematician and a mad scientist. While still enjoying the art of photography and image editing (which drew him to the project early on), he thrives when challenged with designing complex systems and pushing boundaries. His method of building generalized solutions wrapped in elegant layers of abstraction led to his creation of the Graphene engine.

</div>
<div class="block" id="hypercube">

<img src="https://static.graphite.rs/content/about/core-team-photo-hypercube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Hypercube" />

## "Hypercube"

*@0Hypercube* <span class="flag" title="British">🇬🇧</span>

*Editor systems, nodes, tools, architecture*

"Hypercube" is a light speed code monkey who excels at developing, refactoring, and maintaining the editor code base. With an unmatched ability to comprehend intricate code, he delivers lasting and efficient solutions at an impressive pace. He takes ownership of many central editor systems including tools, typography, transforms, layers, and node graph integration.

</div>

</div>

</div>
</section>


<section>

<div class="triptych">

<div class="block">

## Credits

In addition to the work of the Core Team listed above, over a hundred other contributors have written code that makes Graphite what it is today:

<a href="https://github.com/GraphiteEditor/Graphite/graphs/contributors" class="button arrow">Credits</a>

</div>
<div class="block">

## License

The Graphite editor source code is published under the terms of the Apache License 2.0. See below for details and exclusions:

<a href="/license" class="button arrow">License</a>

</div>
<div class="block">

## Logo

More information about the Graphite logo, including its community-focused usage policy and downloadable assets, is available on the logo page:

<a href="/logo" class="button arrow">Logo</a>

</div>

</div>

</section>
