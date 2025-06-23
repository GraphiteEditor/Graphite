+++
title = "Donate"

[extra]
css = ["/page/donate.css", "/component/feature-box.css", "/component/feature-icons.css"]
+++

<section>
<div class="block">

# Funding creativity, not corporations

**Own your tools. Own your art.** Invest in the sustainable, independent future of high-quality creative software that's free, and always will be.

<div class="call-to-action">

<span>
<a href="https://github.com/sponsors/GraphiteEditor" target="_blank" class="button arrow">Donate: GitHub Sponsors</a>
<em>Avoids processing fees</em>
</span>

<span>
<a href="#supporter-memberships" class="button arrow">Donate: without an account</a>
<em>Start to finish in several seconds</em>
</span>

</div>

<div class="feature-icons three-wide statistics" data-statistics>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 34" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
		<span data-statistics-dollars></span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span data-statistics-members></span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 47" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
		<span data-statistics-donors></span>
	</div>
</div>

<script>
(async () => {
	const element = document.querySelector("[data-statistics]");
	const dollarsElement = document.querySelector("[data-statistics-dollars]");
	const membersElement = document.querySelector("[data-statistics-members]");
	const donorsElement = document.querySelector("[data-statistics-donors]");
	if (!dollarsElement || !membersElement || !donorsElement) return;
	try {
		const response = await fetch("https://graphite.rs/sponsorship-stats");
		const json = await response.json();
		if (!json || !json.recurring || !json.one_time_prior_3_month_sum) throw new Error();
		const recurringDollars = parseInt(json.recurring.cents) / 100;
		const oneTimeAverageDollars = parseInt(json.one_time_prior_3_month_sum.cents) / 100 / 3;
		dollarsElement.innerText = "$" + Math.round(recurringDollars + oneTimeAverageDollars).toLocaleString("en-US") + " / month";
		membersElement.innerText = json.recurring.count.toLocaleString("en-US") + " members (supporting monthly)";
		donorsElement.innerText = Math.round(json.one_time_prior_3_month_sum.count / 3).toLocaleString("en-US") + " one-time donors (past month)";
		// Force repaint to work around Safari bug <https://bugs.webkit.org/show_bug.cgi?id=286403> (remove this and its data attribute when the bug is fixed and widely deployed)
		element.style.transform = "scale(1)";
	} catch {
		element.remove();
	}
})();
</script>

Graphite is 100% built and funded by the community. Your contributions directly help us level up the scope and speed of the project's development. Resources are put towards infrastructure, operational costs, swag to keep contributors happy and motivated, and outreach like exhibiting at conventions and traveling to conferences to foster industry relationships. Hiring full-time developers is the next crucial milestone.

</div>
</section>

<section id="supporter-memberships" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">Supporter memberships</h1>

---

Click a membership level below to pay directly by card, no account needed.

A small fee of 3.6% + 30¢ reduces what we receive each month. If convenient, consider instead using <a href="https://github.com/sponsors/GraphiteEditor" target="_blank">GitHub Sponsors</a> for **no fees**.

</div>

<div class="triptych">

<a href="https://buy.stripe.com/6oE2btfCK9863vybII" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">⭕ &ldquo;Quark&rdquo; &raquo;</h1>

**$5 / month**

- Your GitHub profile unlocks a shiny achievement acknowledging your contribution *(through GitHub Sponsors only)*

</a>
<a href="https://buy.stripe.com/00gdUb62aesq9TW7st" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">➕ &ldquo;Proton&rdquo; &raquo;</h1>

**$10 / month**

- Get a **"Member" role** and accompanying **gold-colored nametag on Discord**
- *Plus the lower-tier rewards*

</a>
<a href="https://buy.stripe.com/5kAbM38aiacaeac28a" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">⚛️ &ldquo;Carbon&rdquo; &raquo;</h1>

**$15 / month**

- Your name/handle listed in the end-of-year **retrospective blog post** thank-you section
- *Plus the lower-tier rewards*

</a>

<a href="https://buy.stripe.com/28o4jB62a0BA5DGbIL" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">🧬 &ldquo;DNA&rdquo; &raquo;</h1>

**$25 / month**

- Your **personal name** (or handle) **on the Graphite website and GitHub readme**
- Option to be mailed a personal **thank-you card with Graphite stickers** (in the US only)
- *Plus the lower-tier rewards*

</a>
<a href="https://buy.stripe.com/28o03laiq0BA8PS6os" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">🌱 &ldquo;Organism&rdquo; &raquo;</h1>

**$50 / month**

- Option to be given a public **shout-out of appreciation** from @GraphiteEditor on your choice of social media sites
- *Plus the lower-tier rewards*

</a>
<a href="https://buy.stripe.com/fZedUbduCfwu2ru7sx" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">🌄 &ldquo;Biosphere&rdquo; &raquo;</h1>

**$75 / month**

- Your personal name (or handle) may be a **hyperlink** to your personal site or social media profile
- *Plus the lower-tier rewards*

</a>

</div>

<div class="block action-buttons">

<a href="https://donate.stripe.com/6oU8wP6m0c2kb2AermbQY0a" target="_blank" class="button arrow">Or make a one-time donation</a>

[Manage your ongoing membership](https://billing.stripe.com/p/login/aEU9EzctSfe3cfK5kk)

</div>

</div>
</section>

<section id="corporate-sponsorships" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">Corporate sponsorships</h1>

---

Also available to individuals wanting to make a larger impact. [Reach out](/contact) to pay by invoice or ACH to avoid fees, or for a custom arrangement.

</div>

<div class="triptych">

<a href="https://buy.stripe.com/7sI6rJ1LU5VUaY05kq" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">🪨 &ldquo;Charcoal&rdquo; &raquo;</h1>

**$100 / month**

- Your **company name** may be shown **on the Graphite website and GitHub readme** starting at this tier level
- *Plus the lower-tier rewards for members*

</a>
<a href="https://buy.stripe.com/3cs8zR8ai0BA8PSaEL" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">🛡️ &ldquo;Carbide&rdquo; &raquo;</h1>

**$250 / month**

- Your name on the website and readme may be a **hyperlink** to your company/personal site
- *Plus the lower-tier rewards for members*

</a>
<a href="https://buy.stripe.com/fZeaHZ76e0BAeaccMU" target="_blank" class="block feature-box-narrow">

<h1 class="feature-box-header">💎 &ldquo;Diamond&rdquo; &raquo;</h1>

**$500 / month**

- Your name and link on the website and readme may instead be a **hyperlinked logo**
- *Plus the lower-tier rewards for members*

</a>

</div>

<div class="block action-buttons">

<a href="https://donate.stripe.com/6oU8wP6m0c2kb2AermbQY0a" target="_blank" class="button arrow">Or make a one-time donation</a>

[Manage your ongoing membership](https://billing.stripe.com/p/login/aEU9EzctSfe3cfK5kk)

</div>

</div>
</section>

<!-- <div class="fundraising loading" data-fundraising>
	<div class="fundraising-bar" data-fundraising-bar style="--fundraising-percent: 0%">
		<div class="fundraising-bar-progress"></div>
	</div>
	<div class="goal-metrics">
		<span data-fundraising-percent>Progress: <span data-dynamic>0</span>%</span>
		<span data-fundraising-goal>Goal: $<span data-dynamic>0</span>/month</span>
	</div>
</div> -->
