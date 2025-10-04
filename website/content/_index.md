+++
title = "Free online vector editor & procedural design tool"
template = "section.html"

[extra]
css = ["/page/index.css", "/component/carousel.css", "/component/feature-icons.css", "/component/feature-box.css", "/component/youtube-embed.css"]
js = ["/js/carousel.js", "/js/youtube-embed.js", "/js/video-autoplay.js"]
linked_js = []
meta_description = "Open source free software. A vector graphics creativity suite with a clean, intuitive interface. Opens instantly (no signup) and runs locally in a browser. Exports SVG, PNG, JPG."
+++

<!-- replacements::text_balancer() -->

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

<p class="balance-text">Graphite is a free, open source vector and raster graphics editor, available now in alpha. Get creative with a fully nondestructive editing workflow that combines layer-based compositing with node-based generative design.</p>

</div>
</section>
<!-- ▙ TAGLINE ▟ -->
<!--                -->
<!-- ▛ QUICK LINKS ▜ -->
<section id="quick-links" data-quick-links>

<div class="call-to-action-buttons">
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
		element.innerText = quantity;
		// Force repaint to work around Safari bug <https://bugs.webkit.org/show_bug.cgi?id=286403> (remove this and its data attribute when the bug is fixed and widely deployed)
		document.querySelector("[data-quick-links]").style.transform = "scale(1)";
	} catch {
		element.remove();
	}
})();
</script>
<!-- ▙ QUICK LINKS ▟ -->

<!-- ▛ SCREENSHOTS ▜ -->
<section id="screenshots" class="carousel window-size-1" data-carousel data-carousel-jostle-hint>

<div class="carousel-slide" data-carousel-slide>
	<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__8.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<!-- Above is a copy of the last -->
	<img onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image width="1920" height="1080" loading="lazy" src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__3.avif" />
	<img onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image width="1920" height="1080" loading="lazy" src="https://static.graphite.rs/content/index/magazine-page-layout__2.avif" />
	<img onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image width="1920" height="1080" loading="lazy" src="https://static.graphite.rs/content/index/gui-demo-node-graph-isometric-fountain.avif" />
	<img onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image width="1920" height="1080" loading="lazy" src="https://static.graphite.rs/content/index/gui-demo-fractal__3.avif" />
	<img onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image width="1920" height="1080" loading="lazy" src="https://static.graphite.rs/content/index/gui-mockup-nodes__8.avif" />
	<!-- Below is a copy of the first -->
	<img src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
</div>

<div class="carousel-slide torn left" data-carousel-slide-torn-left></div>
<div class="carousel-slide torn right" data-carousel-slide-torn-right></div>

<div class="screenshot-details">

<div class="carousel-controls">

<button class="direction prev" data-carousel-prev aria-label="Move to previous screenshot">

<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">

<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
<polygon points="24.71,10.71 23.29,9.29 12.59,20 23.29,30.71 24.71,29.29 15.41,20" />

</svg>

</button>
<button class="dot active" data-carousel-dot aria-label="Move to screenshot 1"></button>
<button class="dot" data-carousel-dot aria-label="Move to screenshot 2"></button>
<button class="dot" data-carousel-dot aria-label="Move to screenshot 3"></button>
<button class="dot" data-carousel-dot aria-label="Move to screenshot 4"></button>
<button class="dot" data-carousel-dot aria-label="Move to screenshot 5"></button>
<button class="direction next" data-carousel-next aria-label="Move to next screenshot">

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
	<a href="https://editor.graphite.rs/#demo/isometric-fountain"><em>Isometric Fountain</em></a> — All layer stacks are represented, under the hood, by a nondestructive node graph
</p>
<p data-carousel-description>
	Mandelbrot fractal filled with a noise pattern, procedurally generated and infinitely scalable
</p>
<p data-carousel-description>
	Coming soon: this user interface mockup shows the raster image editing features planned for 2025
</p>

</div>

</div>
</section>
<!-- ▙ SCREENSHOTS ▟ -->
<!--                  -->
<!-- ▛ WHAT'S NEW ▜ -->
<section id="what-is-new" class="block">

<div class="block">

## What's new?

The latest major update is out now! See what the team has been cooking up recently:

<div class="block video-container">
<div>
<div class="youtube-embed aspect-16x9">
	<img data-youtube-embed="Vl5BA4g3QXM" loading="lazy" src="https://static.graphite.rs/content/index/video-september-025-update.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="September Update - Graphite, the Open Source 2D Graphics Suite" />
</div>
</div>
</div>

</div>

</section>
<!-- ▙ WHAT'S NEW ▟ -->
<!--                 -->
<!-- ▛ OVERVIEW ▜ -->
<section id="overview" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">Software overview</h1>

---

<div class="diptych sizzle-video">
<div class="block text">

Starting life as a vector editor, Graphite is evolving into a general-purpose, all-in-one graphics toolbox that is built more like a game engine than a conventional creative app. The editor's tools wrap its node graph core, providing user-friendly workflows for vector, raster, animation, and beyond.

<a href="https://editor.graphite.rs" class="button arrow">Start creating</a>

</div>
<div class="block video">

<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play preload="none" poster="https://static.graphite.rs/content/index/sizzle-compilation-poster.avif">
	<source src="https://static.graphite.rs/content/index/sizzle-compilation.webm" type="video/webm" />
	<source src="https://static.graphite.rs/content/index/sizzle-compilation.mp4" type="video/mp4" />
</video>

</div>
</div>

</div>
<div class="block workflows">

## One app to rule them all

Stop jumping between programs. Upcoming tools will make Graphite a first-class content creation suite for many workflows, including:

<div class="feature-icons stacked no-background">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Graphic Design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Motion Graphics</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 13" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Image Editing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Digital Painting</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Page Layout & Print</span>
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

Where's the download? The web app is [currently live](https://editor.graphite.rs) and desktop apps for Windows, Mac, and Linux will be available in Q4 2025.

Graphite is designed principally as a professional desktop application that is also accessible in a browser for quick access from anywhere. It's built for speed with (nearly) no JavaScript. And regardless of platform, it runs locally and privately on your own hardware— there is no server.

<a href="https://github.com/GraphiteEditor/Graphite/issues/2535" target="_blank">Engineering the tech</a> for a native app distributed across three new platforms takes extra time. That's why supporting the web platform, which keeps up-to-date and reaches all devices, has been the initial target. For now, you can <a href="https://support.google.com/chrome/answer/9658361" target="_blank">install the app as a PWA</a> for a desktop-like experience.

Graphite's code architecture is structured to deliver true native performance for your graphically intensive workloads on desktop platforms and very low overhead on the web thanks to WebAssembly and WebGPU, new high-performance browser technologies.

</div>

</div>
</section>
<!-- ▙ OVERVIEW ▟ -->
<!--               -->
<!-- ▛ DONATE ▜ -->
<section id="donate" class="block">

<div class="block">

<h2 class="heart">Support the mission</h2>

Free software doesn't grow on trees! Chip in your share of the (very real) development costs so you're not leaving others to pick up your tab. In just a few clicks, becoming a member (or giving a one-time donation) lets you help maintain Graphite's sustainability and independence.

<a href="/donate" class="button arrow">Donate now</a>

</div>

</section>
<!-- ▙ DONATE ▟ -->
<!--                 -->
<!-- ▛ PROCEDURALISM ▜ -->
<section id="proceduralism" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">The power of proceduralism</h1>

---

Graphite is the first and only comprehensive graphic design suite built for procedural editing — where everything you make is nondestructive.

</div>

<div class="diptych red-dress">

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play preload="none" poster="https://static.graphite.rs/content/index/procedural-demo-red-dress-poster.avif">
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.mp4" type="video/mp4" />
	</video>
</div>

<div class="block description">

<h1 class="feature-box-header balance-text">Explore parametric possibilities</h1>

Save hours on tedious alterations and make better creative choices. Graphite lets you iterate rapidly by adjusting node parameters instead of individual elements.

Scatter circles with just a couple nodes...  
Want them denser? Bigger? Those are sliders.  
Want a different placement area? Just tweak the path.

<a href="https://editor.graphite.rs/#demo/red-dress">Open this artwork</a> and give it a try yourself.

</div>

</div>
<div class="diptych leaves">

<div class="block description">

<h1 class="feature-box-header balance-text">Mix and morph anything</h1>

Nondestructive editing means every decision is tied to a parameter you can adjust later on. Use Graphite to interpolate between any states just by dragging value sliders.

Blend across color schemes. Morph shapes before they're scattered around the canvas. The options are endless.

<a href="https://editor.graphite.rs/#demo/changing-seasons">Open this artwork</a> and give it a try yourself.

</div>

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play preload="none" poster="https://static.graphite.rs/content/index/procedural-demo-leaves-poster.avif">
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
		<span class="balance-text">Infinitely pan and zoom; export any resolution with no pixelation</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Build workflows to mix AI and human-authored content <em>(future)</em></span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Deploy asset pipelines in studio production environments <em>(future)</em></span>
	</div>
</div>

</div>

</div>
</section>
<!-- ▙ PROCEDURALISM ▟ -->
<!--                   -->
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
	<img src="https://static.graphite.rs/icons/discord__2.svg" alt="" />
	<span class="link not-uppercase arrow">Discord</span>
</a>
<a href="https://www.reddit.com/r/graphite/" target="_blank">
	<img src="https://static.graphite.rs/icons/reddit__3.svg" alt="" />
	<span class="link not-uppercase arrow">Reddit</span>
</a>
<a href="https://bsky.app/profile/graphiteeditor.bsky.social" target="_blank">
	<img src="https://static.graphite.rs/icons/bluesky.svg" alt="" />
	<span class="link not-uppercase arrow">Bluesky</span>
</a>
<a href="https://twitter.com/graphiteeditor" target="_blank">
	<img src="https://static.graphite.rs/icons/twitter.svg" alt="" />
	<span class="link not-uppercase arrow">Twitter</span>
</a>
<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
	<img src="https://static.graphite.rs/icons/youtube.svg" alt="" />
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
<div class="youtube-embed aspect-16x9">
	<img data-youtube-embed="7gjUhl_3X10" loading="lazy" src="https://static.graphite.rs/content/learn/introduction/tutorial-1-vector-art-quickstart-youtube__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector Art Quickstart - Graphite, the Open Source 2D Graphics Suite" />
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
<div class="youtube-embed aspect-16x9">
	<img data-youtube-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
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
[Learn how](/learn/node-graph/imaginate) it works.

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
