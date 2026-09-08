+++
title = "Donate"

[extra]
css = ["/page/donate.css", "/component/feature-box.css"]
js = ["/js/page/donate.js"]
+++

<section>

<div id="donation-thanks"><strong>Thank you for your donation!</strong> Your receipt is on its way by email, and your support goes straight into building Graphite.</div>

<div id="membership-welcome"><strong>Welcome, and thank you for becoming a member!</strong> Your receipt is on its way by email. We're fulfilling membership rewards by hand for now, so please forward that receipt to <a href="mailto:contact@graphite.art">contact<wbr />@graphite<wbr />.art</a> with your Discord username, the name you'd like in the credits, and/or any other details pertinent to your membership level.</div>

<div class="diptych donate-hero">
<div class="block">

# Funding creativity, not corporations

**Own your tools. Own your art.** Graphite is 100% built and funded by our community. Invest in a sustainable, independent future for high-quality creative software that cannot ever be taken away.

<div class="statistics">
	<div class="statistic">
		<span class="value" data-statistics-recurring></span>
		<span class="label">In memberships</span>
		<span class="note">This month</span>
	</div>
	<div class="statistic">
		<span class="value" data-statistics-members></span>
		<span class="label">Members</span>
		<span class="note">This month</span>
	</div>
	<div class="statistic">
		<span class="value" data-statistics-one-time></span>
		<span class="label">In one-time gifts</span>
		<span class="note">Past year</span>
	</div>
	<div class="statistic">
		<span class="value" data-statistics-donors></span>
		<span class="label">One-time donors</span>
		<span class="note">Past year</span>
	</div>
</div>

</div>
<div class="donation-picker" id="supporter-memberships">
	<div class="frequency">
		<label><input type="radio" name="donation-frequency" value="monthly" checked /><span>Monthly</span></label>
		<label><input type="radio" name="donation-frequency" value="one-time" /><span>One-Time</span></label>
	</div>
	<div class="tier-ladder">
		<label class="tier"><input type="radio" name="donation-tier" value="quark" /><span class="name">Quark</span><span class="price">$5</span></label>
		<label class="tier"><input type="radio" name="donation-tier" value="proton" /><span class="name">Proton</span><span class="price">$10</span></label>
		<label class="tier"><input type="radio" name="donation-tier" value="nucleus" /><span class="name">Nucleus</span><span class="price">$15</span></label>
		<label class="tier"><input type="radio" name="donation-tier" value="carbon" checked /><span class="name">Carbon</span><span class="price">$25</span></label>
		<label class="tier"><input type="radio" name="donation-tier" value="charcoal" /><span class="name">Charcoal</span><span class="price">$50</span></label>
		<label class="tier"><input type="radio" name="donation-tier" value="graphite" /><span class="name">Graphite</span><span class="price">$75</span></label>
		<label class="higher-reveal"><input type="checkbox" name="higher-tiers" /><span>Higher tiers with the greatest impact &raquo;</span></label>
		<label class="tier higher"><input type="radio" name="donation-tier" value="graphene" /><span class="name">Graphene</span><span class="price">$100</span></label>
		<label class="tier higher"><input type="radio" name="donation-tier" value="carbide" /><span class="name">Carbide</span><span class="price">$250</span></label>
		<label class="tier higher"><input type="radio" name="donation-tier" value="diamond" /><span class="name">Diamond</span><span class="price">$500</span></label>
	</div>
	<noscript><style>.amount-ladder { display: none !important; }</style></noscript>
	<div class="amount-ladder">
		<button type="button" class="amount" data-amount="25">$25</button>
		<button type="button" class="amount" data-amount="30">$30</button>
		<button type="button" class="amount" data-amount="60">$60</button>
		<button type="button" class="amount" data-amount="100">$100</button>
		<button type="button" class="amount" data-amount="250">$250</button>
		<button type="button" class="amount" data-amount="500">$500</button>
	</div>
	<p class="corporate-link">Sponsoring on behalf of a company? <a href="/contact">Ask about corporate sponsorship.</a></p>
	<div class="picker-body">
		<div class="payment-controls">
			<a href="https://billing.stripe.com/p/login/aEU9EzctSfe3cfK5kk" target="_blank" class="manage">Manage your ongoing membership</a>
			<div class="payment-method">
				<label><input type="radio" name="donation-method" value="direct" checked /><span>Direct Payment</span></label>
				<label><input type="radio" name="donation-method" value="github" /><span>GitHub Sponsors</span></label>
			</div>
		</div>
		<div class="tier-panel" data-tier="quark">
			<div class="rewards">
				<ul>
					<li class="unlocked">Support Graphite's development</li>
					<li class="unlocked github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/6oE2btfCK9863vybII" class="button arrow action" data-method="direct">Donate $5 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333371" target="_blank" class="button arrow action" data-method="github">Donate $5 monthly</a>
		</div>
		<div class="tier-panel" data-tier="proton">
			<div class="rewards">
				<ul>
					<li class="unlocked">Members-only Discord channel and gold nametag</li>
					<li class="unlocked">Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/00gdUb62aesq9TW7st" class="button arrow action" data-method="direct">Donate $10 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333372" target="_blank" class="button arrow action" data-method="github">Donate $10 monthly</a>
		</div>
		<div class="tier-panel" data-tier="nucleus">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/5kAbM38aiacaeac28a" class="button arrow action" data-method="direct">Donate $15 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333373" target="_blank" class="button arrow action" data-method="github">Donate $15 monthly</a>
		</div>
		<div class="tier-panel" data-tier="carbon">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/28o4jB62a0BA5DGbIL" class="button arrow action" data-method="direct">Donate $25 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333374" target="_blank" class="button arrow action" data-method="github">Donate $25 monthly</a>
		</div>
		<div class="tier-panel" data-tier="charcoal">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/28o03laiq0BA8PS6os" class="button arrow action" data-method="direct">Donate $50 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333375" target="_blank" class="button arrow action" data-method="github">Donate $50 monthly</a>
		</div>
		<div class="tier-panel" data-tier="graphite">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/fZedUbduCfwu2ru7sx" class="button arrow action" data-method="direct">Donate $75 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333376" target="_blank" class="button arrow action" data-method="github">Donate $75 monthly</a>
		</div>
		<div class="tier-panel" data-tier="graphene">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li class="unlocked">Custom hyperlink on your site credits name<span class="info" title="Your link is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/7sI6rJ1LU5VUaY05kq" class="button arrow action" data-method="direct">Donate $100 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=333377" target="_blank" class="button arrow action" data-method="github">Donate $100 monthly</a>
		</div>
		<div class="tier-panel" data-tier="carbide">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li class="unlocked">Custom avatar picture with your site credits name<span class="info" title="Your avatar is subject to reasonable content approval."></span></li>
					<li>Custom hyperlink on your site credits name<span class="info" title="Your link is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/3cs8zR8ai0BA8PSaEL" class="button arrow action" data-method="direct">Donate $250 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=441886" target="_blank" class="button arrow action" data-method="github">Donate $250 monthly</a>
		</div>
		<div class="tier-panel" data-tier="diamond">
			<div class="rewards">
				<ul>
					<li class="unlocked">Your name higher in our update video and site credits<span class="info" title="Shown in the credits of our YouTube update videos and on this website. Your name is subject to reasonable content approval."></span></li>
					<li class="unlocked">A spoken personal thank-you during update video credits</li>
					<li>Custom avatar picture with your site credits name<span class="info" title="Your avatar is subject to reasonable content approval."></span></li>
					<li>Custom hyperlink on your site credits name<span class="info" title="Your link is subject to reasonable content approval."></span></li>
					<li>Members-only Discord channel and gold nametag</li>
					<li>Regular dev sneak-peeks and project files from recent YouTube uploads<span class="info" title="Posted as pinned messages in the members-only Discord channel."></span></li>
					<li class="github-only">Shiny GitHub profile achievement<span class="info" title="Awarded for supporting through GitHub Sponsors, unless you opt for private payments."></span></li>
				</ul>
			</div>
			<a href="https://buy.stripe.com/fZeaHZ76e0BAeaccMU" class="button arrow action" data-method="direct">Donate $500 monthly</a>
			<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?tier_id=441888" target="_blank" class="button arrow action" data-method="github">Donate $500 monthly</a>
		</div>
		<div class="tier-panel" data-frequency="one-time">
			<div class="rewards">
				<ul>
					<li>Select a common amount from above or customize it below</li>
					<li>We're currently not set up to fulfill membership rewards for one-time donors, but your selfless generosity is appreciated</li>
				</ul>
			</div>
			<form class="one-time-entry" method="POST" action="https://graphite.art/donate/create-donation-session">
				<div class="amount-entry">
					<label class="amount-field"><span class="currency">$</span><input type="number" name="amount" value="100" min="3" max="10000" step="0.01" required aria-label="Donation amount in US dollars" data-donation-amount /></label>
					<button type="submit" class="button arrow action" data-method="direct">Donate</button>
					<a href="https://github.com/sponsors/GraphiteEditor/sponsorships?frequency=one-time&amp;amount=100" target="_blank" class="button arrow action" data-method="github" data-github-one-time>Donate</a>
				</div>
			</form>
		</div>
	</div>
	<div class="picker-footer">
		<p class="method-note" data-method="direct">Pay by card or other methods in just a few clicks, no account needed.</p>
		<p class="method-note" data-method="github">GitHub subsidizes our processing fees so your full amount reaches us.</p>
	</div>
</div>
</div>
</section>
