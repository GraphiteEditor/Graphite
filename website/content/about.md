+++
title = "About Graphite"

[extra]
css = ["about.css"]
+++

<section>
<div class="section">

# About Graphite

Graphite is a community-built, open source software project that is free to use for any purpose. If you find Graphite valuable, consider [supporting financially](/donate) or [getting involved](/volunteer).

</div>
</section>

<section>
<div class="section">

## Project

The idea for Graphite began with a desire to create artwork and edit photos using free software that felt user-friendly and truly modern. Over time, that dream evolved to reconsider what "modern" meant for the landscape of 2D graphics editing. By borrowing concepts popular in 3D software, what could a procedural, nondestructive design tool look like if nothing was too ambitious? Answering that question took years of design exploration, leading to a community of savvy developers volunteering to turn that formidable dream into a reality.

</div>
</section>

<section>

<div class="diptych">

<div class="section">

## Mission

Graphite strives to unshackle the creativity of every budding artist and seasoned professional by building the best comprehensive art and design tool that's accessible to all.

Mission success will come when Graphite is an industry standard. A cohesive product vision and focus on innovation over imitation is the strategy that will make that possible.

</div>
<div class="section">

## Commitment

As an independent community-driven software project, Graphite will always remain free. It has no investors to answer to. Its founder keeps costs low and relies on [your support](/donate) while he works full-time bringing Graphite to life. To sustainably grow, the long-term funding model will pair donations with paid accounts that provide optional online services like document storage/syncing and render acceleration via cloud GPUs.

</div>

</div>

</section>

<!-- A batteries-included creative app for every kind of digital artist where -->

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
<div class="section">

## A 2D creative tool made for everyone

With great power comes great accessibility. Graphite is built on the belief that the best creative tools can be powerful and within reach of all, from students to studios.

Graphite is designed with a friendly and intuitive interface where a delightful user experience is of first-class importance. It is available for free under an open source [license](/license) and usable [instantly through a web browser](https://editor.graphite.rs) or an upcoming native client on Windows, Mac, and Linux.

It's easy to learn and teach, yet Graphite's accessible design does not sacrifice versatility for simplicity. The node-based workflow opens doors to an ecosystem of powerful capabilities catering to casual and professional users alike.

</div>
<div class="graphic">
	<img src="https://static.graphite.rs/content/index/brush__2.svg" alt="" />
</div>
</section> -->

<section id="core-team" class="feature-box">
<div class="box">

<h1 class="box-header">Meet the core team</h1>

---

<div class="triptych">

<div class="section" id="keavon">

<img src="https://static.graphite.rs/content/about/core-team-photo-keavon-chambers.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Keavon Chambers" />

## Keavon Chambers <span class="flag" title="American">🇺🇸</span>

*@Keavon*

### Founder

*UI & product design, frontend engineering*

Keavon is a creative generalist with a love for the fusion of arts and technology. Photographer, UX and graphic designer, technical artist, game developer, and everything in-between— he is equal parts designer and engineer. His multidisciplinary background in the digital arts is aptly suited for concocting the unique vision needed to bring Graphite to fruition.

</div>
<div class="section" id="dennis">

<img src="https://static.graphite.rs/content/about/core-team-photo-dennis-kobert.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Dennis Kobert" />

## Dennis Kobert <span class="flag" title="German">🇩🇪</span>

*@TrueDoctor*

### Lead Engineer

*Graphene node engine, research, architecture*

Dennis is a mix between a mathematician and a mad scientist. While still enjoying the art of photography and image editing (which drew him to the project early on), he thrives when challenged with designing complex systems and pushing boundaries. His method of building generalized solutions wrapped in elegant layers of abstraction led to his creation of the Graphene engine.

</div>
<div class="section" id="hypercube">

<img src="https://static.graphite.rs/content/about/core-team-photo-hypercube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Photo of Hypercube" />

## "Hypercube" <span class="flag" title="British">🇬🇧</span>

*@0Hypercube*

### Software Engineer

*Editor systems, nodes, architecture*

"Hypercube" is a light speed code monkey who excels at developing, refactoring, and maintaining the editor code base. With an unmatched ability to comprehend intricate code, he delivers lasting and efficient solutions at an impressive pace. He takes ownership of many central editor systems including tools, typography, transforms, layers, and node graph integration.

</div>

</div>

</div>
</section>


<section>

<div class="triptych">

<div class="section">

## Credits

In addition to the work of the Core Team listed above, dozens of contributors have written code that makes Graphite what it is today:

<a href="https://github.com/GraphiteEditor/Graphite/graphs/contributors" class="button arrow">Credits</a>

</div>
<div class="section">

## License

The Graphite editor and source code are provided under the Apache License 2.0 terms. See below for details and exclusions:

<a href="/license" class="button arrow">License</a>

</div>
<div class="section">

## Logo

More information about the Graphite logo, including its community-focused usage policy and downloadable assets, is available on the logo page:

<a href="/logo" class="button arrow">Logo</a>

</div>

</div>

</section>
