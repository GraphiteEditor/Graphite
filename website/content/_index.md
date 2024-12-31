+++
title = "Web-based vector graphics editor and design tool"
template = "section.html"

[extra]
css = ["index.css"]
js = ["image-interaction.js", "video-embed.js"]
+++

<!-- ▛ LOGO ▜ -->
<section id="logo">
<div class="block">
	<img src="https://static.graphite.rs/logos/graphite-logotype-color.svg" alt="Graphite Logo" />
</div>
</section>
<!-- ▙ LOGO ▟ -->

<!-- ▛ TAGLINE ▜ -->
<section id="tagline">
<div class="block">

<h1 class="balance-text">Your <span>procedural</span> toolbox for 2D content creation</h1>

<p class="balance-text">Graphite is a free, open source vector and raster graphics engine, available now in alpha. Get creative with a nondestructive editing workflow that combines layer-based compositing with node-based generative design.</p>

</div>
</section>
<!-- ▙ TAGLINE ▟ -->
<!--                -->
<!-- ▛ QUICK LINKS ▜ -->
<section id="quick-links">

<div class="call-to-action-buttons">
	<style>
	.call-to-action-buttons {
		position: relative;
	}
	.call-to-action-buttons > img {
		position: absolute;
		width: 80%;
		right: 0;
		top: calc(100% + 4px);
		mask: linear-gradient(90deg, #000 30%, #0005, #000 70%) right / 300% 100%;
		animation: shimmer 2s infinite;
	}
	@keyframes shimmer {
		100% {
			mask-position: left;
		}
	}
	@media screen and (max-width: 1080px) {
		.call-to-action-buttons > img {
			display: none;
		}
	}
	</style>
	<img src="data:image/png;base64,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" alt="" />
	<a href="https://github.com/GraphiteEditor/Graphite" class="button github-stars">
		<img src="https://static.graphite.rs/icons/github.svg" alt="GitHub" />
		<span class="arrow">Star</span>
		<div data-github-stars></div>
	</a>
	<a href="#newsletter" class="button arrow">Subscribe to newsletter</a>
</div>
<div class="social-media-buttons">
	<a href="https://discord.graphite.rs" target="_blank">
		<img src="https://static.graphite.rs/icons/discord__2.svg" alt="Discord" />
	</a>
	<a href="https://www.reddit.com/r/graphite/" target="_blank">
		<img src="https://static.graphite.rs/icons/reddit__3.svg" alt="Reddit" />
	</a>
	<a href="https://bsky.app/profile/graphiteeditor.bsky.social" target="_blank">
		<img src="https://static.graphite.rs/icons/bluesky.svg" alt="Bluesky" />
	</a>
	<a href="https://twitter.com/graphiteeditor" target="_blank">
		<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
	</a>
	<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
		<img src="https://static.graphite.rs/icons/youtube.svg" alt="YouTube" />
	</a>
</div>

</section>

<script>
(async () => {
	const element = document.querySelector("[data-github-stars]");
	try {
		const response = await fetch("https://api.github.com/repos/graphiteeditor/graphite?per_page=1");
		const json = await response.json();
		const stars = parseInt(json.stargazers_count);
		if (!stars) throw new Error();
		let quantity = stars.toLocaleString("en-US");
		if (quantity.length === 5) quantity = quantity.replace(",", "");
		element.innerText = quantity;
	} catch {
		element.remove();
	}
})();
</script>
<!-- ▙ QUICK LINKS ▟ -->

<!-- ▛ SCREENSHOTS ▜ -->
<section id="screenshots" class="carousel window-size-1" data-carousel data-carousel-jostle-hint>

<div class="carousel-slide" data-carousel-slide>
	<!-- Copy of last --><img src="https://static.graphite.rs/content/index/gui-mockup-nodes__7.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/magazine-page-layout.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-node-graph-valley-of-spires__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-fractal__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__7.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<!-- Copy of first --><img src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
</div>

<div class="carousel-slide torn left" data-carousel-slide-torn-left></div>
<div class="carousel-slide torn right" data-carousel-slide-torn-right></div>

<div class="screenshot-details">

<div class="carousel-controls">

<button class="direction prev" data-carousel-prev>

<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">

<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
<polygon points="24.71,10.71 23.29,9.29 12.59,20 23.29,30.71 24.71,29.29 15.41,20" />

</svg>

</button>
<button class="dot active" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="direction next" data-carousel-next>

<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">

<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
<polygon points="16.71,9.29 15.29,10.71 24.59,20 15.29,29.29 16.71,30.71 27.41,20" />

</svg>

</button>

</div>
<div class="screenshot-description">

<p data-carousel-description class="active">
	<a href="https://editor.graphite.rs/#demo/painted-dreams"><em>Painted Dreams</em></a> — Made using nondestructive boolean operations and procedural polka dot patterns
</p>
<p data-carousel-description>
	Design for a magazine spread, a preview of the upcoming focus on desktop publishing
</p>
<p data-carousel-description>
	<a href="https://editor.graphite.rs/#demo/valley-of-spires"><em>Valley of Spires</em></a> — All layer stacks are represented, under the hood, by a node graph
</p>
<p data-carousel-description>
	Mandelbrot fractal filled with a noise pattern, procedurally generated and infinitely scalable
</p>
<p data-carousel-description>
	Coming soon: mockup for the actively in-development raster workflow with new nodes for photo editing
</p>

</div>

</div>
</section>
<!-- ▙ SCREENSHOTS ▟ -->
<!--                 -->
<!-- ▛ OVERVIEW ▜ -->
<section id="overview" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">Software overview</h1>

---

<!-- As a new entrant to the open source digital content creation landscape, Graphite has a unique formula for success: -->

Starting life as a vector editor, Graphite is evolving into a generalized, all-in-one graphics toolbox that's built more like a game engine than a conventional creative app. The editor's tools wrap its node graph core, providing user-friendly workflows for vector, raster, and beyond.

</div>
<div class="block workflows">

## One app to rule them all

Stop jumping between programs— upcoming tools will make Graphite a first-class content creation suite for many workflows, including:

<div class="feature-icons stacked no-background">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Graphic Design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 13" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Image Editing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Motion Graphics</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Digital Painting</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Desktop Publishing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>VFX Compositing</span>
	</div>
</div>

</div>
<div class="diptych">

<div class="block">

## Current features

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Vector editing tools</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Procedural workflow for graphic design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Node-based layers</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Forever free and open source</span>
	</div>
</div>

Presently, Graphite is a lightweight offline web app with features primarily oriented around procedural vector graphics editing.

</div>
<div class="block">

## Upcoming features

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>All-in-one creative tool for all things 2D</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Fully-featured raster manipulation</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Windows/Mac/Linux native apps + web</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Live collaborative editing</span>
	</div>
</div>

<a href="/features#roadmap" class="button arrow">Roadmap</a>

</div>

</div>
<div class="block">

## Desktop-first and web-ready

Graphite is designed principally as a professional-grade desktop application that is also accessible in-browser for quick, casual usage.

Where's the download? Windows, Mac, and Linux apps should be available around the end of 2024. Until then, you can <a href="https://support.google.com/chrome/answer/9658361" target="_blank">install it as a PWA</a>.

Developing and maintaining a native app on so many platforms is a big task. A fast, sloppy approach wouldn't cut it, but engineering the right tech takes time. That's why first supporting just web, the one platform that stays up-to-date and reaches all devices, was the initial priority.

Once it's ready to shine, Graphite's code architecture is structured to deliver native performance for your graphically intensive workloads on desktop platforms and very low overhead on the web thanks to WebAssembly and WebGPU, new high-performance browser technologies.

</div>

</div>
</section>
<!-- ▙ OVERVIEW ▟ -->
<!--                  -->
<!-- ▛ PROCEDURALISM ▜ -->
<section id="proceduralism" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">The power of proceduralism</h1>

---

Graphite is the first and only graphic design package built for procedural editing — where everything you make is nondestructive.

</div>

<div class="diptych red-dress">

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.mp4" type="video/mp4" />
	</video>
</div>

<div class="block description">

<h1 class="feature-box-header balance-text">Explore creative possibilities</h1>

Save hours on tedious alterations and make better creative choices. Graphite lets you iterate rapidly by adjusting node parameters instead of individual elements.

Scatter circles with just a couple nodes...  
Want them denser? Bigger? Those are sliders.  
Want a different placement area? Just tweak the path.

<a href="https://editor.graphite.rs/#demo/red-dress">Open this artwork</a> and give it a try yourself.

</div>

</div>
<div class="diptych leaves">

<div class="block description">

<h1 class="feature-box-header balance-text">Mix and morph parameters</h1>

Nondestructive editing means every decision is tied to a parameter you can adjust later on. Use Graphite to interpolate between any states just by dragging sliders.

Blend across color schemes. Morph shapes before they're scattered around the canvas. The possibilities are endless.

<a href="https://editor.graphite.rs/#demo/changing-seasons">Open this artwork</a> and give it a try yourself.

</div>

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/procedural-demo-leaves.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/procedural-demo-leaves.mp4" type="video/mp4" />
	</video>
</div>

</div>
<div class="block pipelines">

## Geared for generative pipelines

Graphite's representation of artwork as a node graph lets you customize, compose, reuse, share, and automate your content workflows:

<div class="feature-icons four-wide">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Pixelation-free infinite zooming and panning of boundless content</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Modular node-based pipelines for generative AI <em>(soon)</em></span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Asset pipelines for studio production environments <em>(soon)</em></span>
	</div>
</div>

</div>

</div>
</section>
<!-- ▙ PROCEDURALISM ▟ -->
<!--                 -->
<!-- ▛ DONATE ▜ -->
<section id="donate" class="block">

<div class="block">

## Support the mission

If you aren't paying for your free software, someone else is covering your share. Chip in so Graphite can remain sustainable and independent.

<a href="https://github.com/sponsors/GraphiteEditor" class="button arrow">Donate</a>

</div>

</section>
<!-- ▙ DONATE ▟ -->
<!--                -->
<!-- ▛ NEWSLETTER ▜ -->
<section id="newsletter" class="feature-box-narrow">
<div id="newsletter-success"><!-- Used only as a URL hash fragment anchor --></div>

<div class="diptych">

<div class="block newsletter-signup">

<h1 class="feature-box-header">Stay in the loop</h1>

Subscribe to the newsletter for quarterly updates on major development progress. And follow along, or join the conversation, on social media.

<div class="newsletter-success">

## Thanks!

You'll receive your first newsletter email with the next major Graphite news.

</div>
<form action="https://graphite.rs/newsletter-signup" method="post">
	<div class="same-line">
		<div class="input-column name">
			<label for="newsletter-name">First + last name:</label>
			<input id="newsletter-name" name="name" type="text" required />
		</div>
		<div class="input-column phone">
			<label for="newsletter-phone">Phone:</label>
			<input id="newsletter-phone" name="phone" type="text" tabindex="-1" autocomplete="off" />
		</div>
		<div class="input-column email">
			<label for="newsletter-email">Email address:</label>
			<input id="newsletter-email" name="email" type="email" required />
		</div>
	</div>
	<div class="input-column submit">
		<input type="submit" value="Subscribe" class="button" />
	</div>
</form>

</div>
<div class="block social-media-links">

<a href="https://discord.graphite.rs" target="_blank">
	<img src="https://static.graphite.rs/icons/discord__2.svg" alt="Discord" />
	<span class="link not-uppercase arrow">Discord</span>
</a>
<a href="https://www.reddit.com/r/graphite/" target="_blank">
	<img src="https://static.graphite.rs/icons/reddit__3.svg" alt="Reddit" />
	<span class="link not-uppercase arrow">Reddit</span>
</a>
<a href="https://bsky.app/profile/graphiteeditor.bsky.social" target="_blank">
	<img src="https://static.graphite.rs/icons/bluesky.svg" alt="Bluesky" />
	<span class="link not-uppercase arrow">Bluesky</span>
</a>
<a href="https://twitter.com/graphiteeditor" target="_blank">
	<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
	<span class="link not-uppercase arrow">Twitter</span>
</a>
<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
	<img src="https://static.graphite.rs/icons/youtube.svg" alt="YouTube" />
	<span class="link not-uppercase arrow">YouTube</span>
</a>

</div>

</div>
</section>
<!-- ▙ NEWSLETTER ▟ -->
<!--                -->
<!-- ▛ DIVE IN ▜ -->
<section id="dive-in" class="block">

<div class="block">

## Ready to dive in?

Get started with Graphite by following along to a hands-on quickstart tutorial.

<div class="block video-container">
<div>
<div class="video-embed aspect-16x9">
	<img data-video-embed="7gjUhl_3X10" src="https://static.graphite.rs/content/index/tutorial-1-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite Tutorial 1 - Hands-On Quickstart" />
</div>
</div>
</div>

<div class="buttons">
<a href="https://editor.graphite.rs" class="button arrow">Launch Graphite</a>
<a href="/learn" class="button arrow">Continue learning</a>
</div>

</div>

</section>
<!-- ▙ DIVE IN ▟ -->
<!--                 -->
<!-- ▛ RECENT NEWS ▜ -->
<section id="recent-news" class="feature-box-outer">
<div class="feature-box-inner">

<h1 class="feature-box-header">Recent news <span> / </span> <a href="/blog" class="link arrow">More in the blog</a></h1>

---

<div class="diptych">
<!-- replacements::blog_posts(count = 2) -->
</div>

</div>
</section>
<!-- ▙ RECENT NEWS ▟ -->
<!--                  -->
<!-- ▛ DEMO VIDEO ▜ -->
<!--
<section id="demo-video">
<div class="block">
Watch this timelapse showing the process of mixing traditional vector art (tracing a physical sketch and colorizing it, first two minutes) with using Imaginate to generate a background (last 45 seconds).
<div class="video-embed aspect-16x9">
	<img data-video-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
</div>
(Recorded in an older version of Graphite from early 2023.)
</div>
</section>
-->
<!-- ▙ DEMO VIDEO ▟ -->
<!--                 -->
<!-- ▛ IMAGINATE ▜ -->

<!-- TODO: Reenable when Imaginate is properly working again -->

<!--

<section id="imaginate">

<div class="block">

<h1><span class="alternating-text"><span>Co-create</span><span>Ideate</span><span>Illustrate</span><span>Generate</span><span>Iterate</span></span> with Imaginate</h1>

**Imaginate** is a node powered by <a href="https://en.wikipedia.org/wiki/Stable_Diffusion" target="_blank">Stable Diffusion</a> that makes AI-assisted art creation an easy, nondestructive process.
<!-- [Learn how](/learn/node-graph/imaginate) it works. --////////////////////>

</div>
<div class="diptych">

<div class="block">

<h2 class="balance-text">Add a touch of style</h2>

**Magically reimagine your vector drawings** in a fresh new style. Just place an Imaginate node between your layers and describe how it should end up looking.

<div class="image-comparison" data-image-comparison style="--comparison-percent: 50%">
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/light-bulb-before.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector illustration of a light bulb" />
	</div>
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/light-bulb-after.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Watercolor painting of a light bulb" />
	</div>
	<div class="slide-bar">
		<div class="arrows">
			<div></div>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
		</div>
	</div>
</div>

<blockquote class="balance-text require-polyfill"><strong>Watercolor painting</strong> of a light bulb gleaming with an exclamation mark inside</blockquote>

</div>
<div class="block">

## Work fast and sloppy

**Doodle a rough draft** without stressing over the details. Let Imaginate add the finishing touches to your artistic vision. Iterate with more passes until you're happy.

<div class="image-comparison" data-image-comparison style="--comparison-percent: 50%">
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/california-poppies-before.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Sloppy poppy: vector doodle of California poppy flowers wrapped around a circle" />
	</div>
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/california-poppies-after.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Polished poppy: artistic, high-quality illustration of California poppy flowers wrapped around a circle" />
	</div>
	<div class="slide-bar">
		<div class="arrows">
			<div></div>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
		</div>
	</div>
</div>

<blockquote class="balance-text require-polyfill"><strong>Botanical illustration</strong> of California poppies wrapped around a circle</blockquote>

</div>

</div>

</section>

-->

<!-- ▙ IMAGINATE ▟ -->
