+++
title = "Manage membership"
path = "donate/membership"

[extra]
css = ["/page/membership.css", "/component/feature-box.css", "/component/credits-names.css"]
js = ["/js/page/membership.js", "/js/component/credits-particles.js"]
+++

<section>

<div class="diptych membership-hero">
<div class="block">

# Manage membership

Thank you for helping make Graphite possible! Signed-in members can manage how they support the project and access their reward perks on this page.

<div class="billing-note" data-billing-provider="stripe" hidden><p>Tier changes take effect right away: upgrades charge the price difference for the remainder of the current billing period and downgrades credit the difference toward your next payment. Ending your membership stops future payments, and your rewards last through the period you've already paid for.</p></div>

<div class="billing-note" data-billing-provider="github" hidden><p>Manage your sponsorship on <a href="https://github.com/sponsors/GraphiteEditor">GitHub Sponsors</a>. Upgrades take effect right away. Downgrading or ending your sponsorship takes effect on your next GitHub billing date, and you keep your current rewards until then.</p></div>

</div>
<div class="membership-panel">
	<div class="signed-out" data-signed-out>
		<div class="notices">
			<div class="notice" data-notice="sign-in-expired" hidden>That sign-in timed out. Please try again.</div>
			<div class="notice" data-notice="sign-in-failed" hidden>Something went wrong while signing you in. Please try again.</div>
			<div class="notice" data-notice="session-expired" hidden>You've been signed out, so that didn't go through. Please sign in and try again.</div>
			<div class="notice confirmation" data-notice="sign-in-link-sent" hidden>If that email address has an active membership with direct payments (not GitHub Sponsors), a sign-in link is on its way.</div>
			<div class="notice" data-notice="sign-in-link-expired" hidden>That sign-in link has expired. Enter your email address below for a new one.</div>
			<div class="notice" data-notice="email-change-sign-in" hidden>To finish changing your email address, sign in below with the one your membership uses now.</div>
			<div class="notice" data-notice="email-change-expired" hidden>That confirmation link no longer works. To change your email address, ask for a new link while signed in.</div>
		</div>
		<h2>How are you paying for your membership?</h2>
		<div class="payment-method">
			<label><input type="radio" name="payment-method" value="direct" /><span>Direct payment</span></label>
			<label><input type="radio" name="payment-method" value="github" /><span>GitHub Sponsors</span></label>
		</div>
		<form class="sign-in direct" method="POST" action="/donate/membership/login/email" data-email-sign-in>
			<label class="field">
				<span class="field-label">Email address used at checkout</span>
				<input type="email" name="email" maxlength="254" autocomplete="email" required data-sign-in-address />
			</label>
			<button type="submit" class="button arrow" data-sign-in-button>Email me a sign-in link</button>
			<div class="aside" data-notice="sign-in-link-sent" hidden><strong>Not getting our sign-in email?</strong> First, confirm it didn't land in a spam or promotions folder, and that you're not actually paying through GitHub Sponsors. Otherwise, <a href="/contact">contact us</a> for help signing in to set up your rewards. Meanwhile, you can still cancel, change tiers, or update payment methods through <a href="https://billing.stripe.com/p/login/aEU9EzctSfe3cfK5kk">Stripe's billing portal</a>, which sends its own sign-in email.</div>
		</form>
		<div class="sign-in github">
			<a href="/donate/membership/login/github" class="button arrow">Sign in with GitHub</a>
		</div>
		<p class="aside">Not a member yet? <a href="/donate/">Pick a tier.</a></p>
	</div>
	<div class="signed-in" data-signed-in hidden>
		<div class="tier" data-tier-heading>
			<span class="tier-name" data-tier-name></span>
			<span class="tier-terms"><span data-monthly-price></span> monthly <span data-paid-through></span></span>
		</div>
		<div class="account">
			<span class="account-name">Signed in <span data-sign-in-method>with GitHub</span> as <strong data-account-name></strong></span>
			<button type="button" class="text-button" aria-expanded="false" data-change-email-toggle hidden>Change email</button>
			<form method="POST" action="/donate/membership/logout"><button type="submit" class="text-button">Sign out</button></form>
		</div>
		<form class="change-email" method="POST" action="/donate/membership/email" data-change-email hidden>
			<label class="field">
				<span class="field-label">New email address</span>
				<input type="email" name="email" maxlength="254" autocomplete="email" required data-new-email-address />
				<span class="field-hint">Your membership, receipts, and sign-in move to this address once you confirm it with the link we email there</span>
			</label>
			<button type="submit" class="button arrow">Email me a confirmation link</button>
		</form>
		<div class="notices">
			<form class="notice confirmation email-change-confirmation" method="POST" action="/donate/membership/verify/new-email" data-email-change-confirmation hidden>
				<span>Make <strong data-pending-email></strong> the email address for your Graphite membership?</span>
				<button type="submit" class="button arrow">Confirm new email address</button>
			</form>
			<div class="notice" data-notice="email-change-expired" hidden>That confirmation link no longer works. To change your email address, ask for a new link while signed in.</div>
			<div class="notice" data-notice="email-taken" hidden>That email address already has its own membership, so yours stayed where it was. Sign in with that address to manage its membership, or <a href="/contact">contact us</a> to combine the two.</div>
			<div class="notice" data-notice="email-change-failed" hidden>Your email address couldn't be changed. Please try again in a moment.</div>
			<div class="notice" data-notice="save-failed" hidden>Your changes couldn't be saved. Please try again in a moment.</div>
			<div class="notice" data-notice="link-invalid" hidden>That link isn't a web address, so your changes weren't saved.</div>
			<div class="notice" data-notice="avatar-invalid" hidden>That picture couldn't be used, so your changes weren't saved. It needs to be a still PNG, JPEG, or WebP under 512 KB and no larger than 256x256 pixels.</div>
			<div class="notice" data-notice="discord-failed" hidden>Something went wrong with Discord. Please try again.</div>
			<div class="notice" data-notice="billing-failed" hidden>We couldn't open your billing settings. Please try again, or go straight to <a href="https://billing.stripe.com/p/login/aEU9EzctSfe3cfK5kk">Stripe's billing portal</a>.</div>
			<div class="notice" data-notice="email-invalid" hidden>That isn't an email address, so nothing changed.</div>
			<div class="notice confirmation" data-notice="email-change-sent" hidden>A confirmation link is on its way to your new email address. Your address changes once you confirm with that link.</div>
			<div class="notice confirmation passing" role="status" data-notice="email-changed" hidden>Your membership now uses your new email address.</div>
			<div class="notice confirmation passing" role="status" data-notice="saved" hidden>Your changes are saved.</div>
			<div class="notice confirmation passing" role="status" data-notice="discord-linked" hidden>Your Discord account is linked, and your gold nametag is ready.</div>
		</div>
		<div class="membership" data-membership-state="active">
			<div class="billing" data-billing-provider="stripe">
				<h2>Billing</h2>
				<p class="notice" data-payment-failed hidden>Your membership payment didn't go through. Update your payment details below to keep your membership and rewards going.</p>
				<p class="notice" data-duplicate-memberships hidden>This email address has <span data-membership-count></span> memberships, which add up to your tier. If you meant to have only one, end the others.</p>
				<div class="stripe-memberships" data-stripe-memberships></div>
			</div>
			<div class="billing" data-billing-provider="github">
				<h2>Billing</h2>
				<a href="https://github.com/sponsors/GraphiteEditor" class="button arrow" data-change-tier>Change or cancel on GitHub</a>
			</div>
			<div class="discord" data-reward-tier="quark">
				<h2>Discord perks</h2>
				<div class="field">
					<span class="field-label">Linked account</span>
					<a href="/donate/membership/link/discord" class="button arrow" data-discord-unlinked>Link Discord</a>
					<form class="linked-account" method="POST" action="/donate/membership/unlink/discord" data-discord-linked hidden>
						<span class="linked-account-name" data-discord-username></span>
						<button type="submit" class="text-button">Unlink</button>
					</form>
					<span class="field-hint">Gain access to our members-only channel with regular dev sneak peeks and project files from recent YouTube uploads, plus a gold nametag</span>
				</div>
			</div>
			<form class="credits" method="POST" action="/donate/membership/save" enctype="multipart/form-data" data-credits-form>
				<h2>Credits</h2>
				<div class="aside" data-credits-locked><a href="/donate/" data-upgrade-button>Upgrade your tier to Proton ($10/month)</a> to have your name included in the credits.</div>
				<button type="button" class="link-button preview-toggle" data-try-higher-tiers hidden>Want to preview the personalization available at higher tiers?</button>
				<button type="button" class="link-button preview-toggle" data-stop-previewing hidden>End previewing the personalization available at higher tiers.</button>
				<div class="preview-notice" data-preview-notice hidden>
					<template data-preview-sentence><span><span data-preview-choices></span> <span data-preview-verb>are</span> just a preview and won't appear in the credits until you <a href="#" data-upgrade-button>upgrade your tier to <span data-needed-tier></span> (<span data-needed-tier-price></span>/month)</a>.</span></template>
					<span data-preview-sentences></span>Or <a href="#" data-stop-previewing>stop previewing</a> to remove <span data-preview-pronoun>them</span>.
				</div>
				<div class="field name-opt-in" data-reward-tier="proton">
					<span class="field-label">Name opt-in</span>
					<div class="checkboxes">
						<label class="checkbox" data-reward-tier="proton"><input type="checkbox" name="show_in_site_credits" data-show-in-site-credits /><span data-reward-label>Show in site credits</span></label>
						<label class="checkbox" data-reward-tier="proton"><input type="checkbox" name="show_in_video_credits" data-show-in-video-credits /><span data-reward-label>Show in YouTube video outro credits</span></label>
						<label class="checkbox" data-reward-tier="diamond"><input type="checkbox" name="speak_in_video_credits" data-speak-in-video-credits /><span data-reward-label>Speak my name in YouTube outro acknowledgments</span></label>
					</div>
				</div>
				<label class="field" data-reward-tier="proton">
					<span class="field-label" data-reward-label>Name to show</span>
					<input type="text" name="credits_name" maxlength="60" autocomplete="name" data-credits-name />
					<span class="field-hint">We may remove names that aren't appropriate for public credits</span>
				</label>
				<label class="field spoken-name-field" data-reward-tier="diamond">
					<span class="field-label" data-reward-label>Name to speak</span>
					<input type="text" name="spoken_name" maxlength="120" data-spoken-name />
					<span class="field-hint">Add how to say it, like "Inara Serra (ih-NAR-uh SAIR-uh)"</span>
				</label>
				<label class="field" data-reward-tier="graphene">
					<span class="field-label" data-reward-label>Link on my name</span>
					<input type="text" name="credits_link" maxlength="200" inputmode="url" autocomplete="url" placeholder="https://" data-credits-link />
				</label>
				<div class="avatar-field" data-reward-tier="graphite">
					<label class="field-label" for="credits-avatar" data-reward-label>Avatar picture</label>
					<div class="avatar-picker">
						<img class="avatar" alt="" width="72" height="72" data-credits-avatar hidden />
						<div class="avatar-controls">
							<div class="file-picker">
								<input type="file" id="credits-avatar" name="credits_avatar" accept="image/png, image/jpeg, image/webp" data-credits-avatar-file />
								<label class="file-picker-button" for="credits-avatar">Choose picture</label>
								<span class="file-picker-status" data-credits-avatar-file-name>No file selected</span>
							</div>
							<label class="checkbox" data-remove-avatar hidden><input type="checkbox" name="remove_credits_avatar" data-remove-avatar-checkbox /><span>Remove my picture</span></label>
						</div>
					</div>
					<div class="avatar-cropper" data-avatar-cropper hidden>
						<div class="crop-frame" tabindex="0" aria-label="Picture crop, moved by dragging or with the arrow keys" data-crop-frame><img alt="" draggable="false" data-crop-image /></div>
						<label class="crop-zoom"><span class="field-label">Zoom</span><input type="range" min="1" max="4" step="0.01" value="1" data-crop-zoom /></label>
						<span class="field-hint">Drag the picture to choose what's shown</span>
					</div>
				</div>
				<div class="credits-preview" data-reward-tier="proton" data-credits-preview hidden>
					<span class="field-label">Site credits preview</span>
					<ul class="credits-names">
						<li data-name-particle-shape="" data-credits-preview-entry><img alt="" width="48" height="48" data-credits-preview-avatar hidden /><span class="credits-name" data-credits-preview-name></span></li>
					</ul>
					<span class="field-hint">Colors, pictures, links, and effects only appear in the site credits</span>
				</div>
				<div class="field" data-reward-tier="nucleus">
					<label class="field-label" for="credits-color-style" data-reward-label>Name color</label>
					<select id="credits-color-style" name="credits_color_style" data-credits-color-style></select>
					<div class="tile-row" data-color-row="credits_colors">
						<span class="tiles" data-color-swatches></span>
						<button type="button" class="tile-button" aria-label="Add a color" data-add-color>+</button>
						<button type="button" class="tile-button" aria-label="Click a color to remove it" aria-pressed="false" data-remove-color>−</button>
						<div class="color-popover" data-color-popover hidden>
							<div class="saturation-value" tabindex="0" role="slider" aria-label="Saturation and brightness" data-saturation-value><span class="saturation-value-handle"></span></div>
							<div class="hue-slider" tabindex="0" role="slider" aria-label="Hue" aria-orientation="vertical" aria-valuemin="0" aria-valuemax="360" data-hue-slider><span class="hue-handle"></span></div>
							<input type="text" class="hex-field" maxlength="7" spellcheck="false" autocomplete="off" aria-label="Hex code" data-hex-field />
						</div>
					</div>
					<template data-swatch-template><span class="swatch"><input type="color" name="credits_colors" data-color-picker value="#16323f" aria-label="Color" /></span></template>
				</div>
				<label class="field" data-reward-tier="carbide">
					<span class="field-label" data-reward-label>Particle motion</span>
					<select name="credits_particle_motion" data-credits-particle-motion>
						<option value="">None</option>
						<option value="ascent">Ascent</option>
						<option value="blink">Blink</option>
						<option value="bounce">Bounce</option>
						<option value="burst">Burst</option>
						<option value="drift">Drift</option>
						<option value="encircle">Encircle</option>
						<option value="fall">Fall</option>
						<option value="gather">Gather</option>
						<option value="levitate">Levitate</option>
						<option value="orbit">Orbit</option>
						<option value="rise">Rise</option>
						<option value="stream">Stream</option>
						<option value="tornado">Tornado</option>
					</select>
				</label>
				<div class="field" data-reward-tier="carbide" data-particle-shapes-field>
					<span class="field-label" data-reward-label>Particle shapes</span>
					<div class="tile-row shape-row" data-shape-row>
						<span class="tiles" data-shape-list></span>
						<button type="button" class="tile-button" aria-label="Add a particle shape" data-add-shape>+</button>
						<button type="button" class="tile-button" aria-label="Click a shape to remove it" aria-pressed="false" data-remove-shape>−</button>
					</div>
					<template data-shape-template><span class="shape-choice"><select name="credits_particle_shapes" aria-label="Particle shape" data-particle-shape><option value="bats">Bats</option><option value="blossoms">Blossoms</option><option value="bubbles">Bubbles</option><option value="butterflies">Butterflies</option><option value="confetti">Confetti</option><option value="crescents">Crescents</option><option value="fireworks">Fireworks</option><option value="flames">Flames</option><option value="gems">Gems</option><option value="hearts">Hearts</option><option value="leaves">Leaves</option><option value="bolts">Lightning bolts</option><option value="notes">Music notes</option><option value="orbs">Orbs</option><option value="petals">Petals</option><option value="pixels">Pixels</option><option value="snowflakes">Snowflakes</option><option value="stars">Stars</option><option value="vectors">Vector paths</option></select></span></template>
				</div>
				<div class="field" data-reward-tier="carbide" data-particle-colors-field>
					<span class="field-label" data-reward-label>Particle colors</span>
					<div class="tile-row" data-color-row="credits_particle_colors">
						<span class="tiles" data-color-swatches></span>
						<button type="button" class="tile-button" aria-label="Add a particle color" data-add-color>+</button>
						<button type="button" class="tile-button" aria-label="Click a particle color to remove it" aria-pressed="false" data-remove-color>−</button>
					</div>
					<span class="field-hint">Leave empty to keep each shape's own colors</span>
				</div>
				<button type="submit" class="button arrow" data-reward-tier="proton" data-save-changes>Save changes</button>
			</form>
		</div>
		<div class="membership" data-membership-state="past-due">
			<p>Your last membership payment didn't go through, so your rewards are paused. Update your payment details below. Your rewards will return automatically once the payment goes through.</p>
			<div class="billing">
				<h2>Billing</h2>
				<div class="stripe-memberships" data-stripe-memberships></div>
			</div>
		</div>
		<div class="membership" data-membership-state="below-tiers">
			<p>Thank you for your <strong data-monthly-price></strong> monthly support! Membership rewards begin at the $5 Quark tier.</p>
			<a href="/donate/" class="button arrow">See the tiers</a>
		</div>
		<div class="membership" data-membership-state="inactive">
			<p>This <span data-account-kind>account</span> doesn't have an active membership. If you're a member through a different <span data-account-kind>account</span>, log out and sign in with that one.</p>
			<p data-organization-sponsors hidden>Sponsoring through an organization? <a href="/contact">Contact us</a> and we'll set up your rewards.</p>
			<a href="/donate/" class="button arrow">Become a member</a>
		</div>
	</div>
</div>
</div>

</section>
