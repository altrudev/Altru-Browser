# Altru.dev Browser Landing Evidence — 2026-10-06

## Public landing

Canonical product landing:

`https://altru.dev/browser`

GitHub repository homepage metadata now points to that URL.

## Governed deployment transaction

Altru Remote transaction:

`060e40d9-8e9d-473d-b8bb-b5906491e900`

Transaction state: **committed**.

Commit receipt:

`4f56a391adf509ad3f5647ffa14c62b007bcd175fab45b27ed637f8511946718`

Changed production files:

- `index.php` → `0c14a09cc6dd65f9b6d79592b7e941b2ed3f4f2d10d8b72c0b450efd80cec4d1`
- `includes/views.php` → `6103a42f590842b71267638184d274d0c79fa061f7d2e9fb674d7a12e4edee7b`
- `assets/css/site.css` → `6333401aea83463b69d31405212e6f84f55f4d388ae1ede0b4d032f4f550021f`
- `assets/brand/altru-browser-desktop.svg` → `f11831054f517c4258b1acfde21ce1b8d1a8e0070510ee2a0917f31b56670bbc`
- `assets/brand/altru-browser-mobile.svg` → `5742cd5112b161af17b689aedd5234f4b3dd9540c1d01afe8ad0e1fd061b016a`

## Live verification

Fresh uncached HTTPS requests confirmed:

- `/browser` renders the Altru Browser product page;
- headline: **The web, without the weight.**
- status label: **Experimental · Community Candidate**;
- current evidence includes 74/74 debug, 74/74 release, and 75/75 Taffy-feature tests;
- desktop and mobile design-direction sections are present;
- featured homepage card now links to `/browser`;
- mark, desktop shell, and mobile shell SVG assets all return HTTP 200 with `image/svg+xml`.

## DNS boundary

`browser.altru.dev` did not resolve during this deployment. The currently authorized Altru Remote surface controls bounded site-file deployment, not DNS/cPanel subdomain provisioning. Therefore `/browser` is the stable canonical landing until a separate DNS/subdomain authority is established.

No DNS state was guessed or modified outside the available authority boundary.
