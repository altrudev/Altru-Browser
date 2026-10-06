# Altru.dev Featured Integration Evidence — 2026-10-06

## Purpose

Publish Altru Browser as the first curated featured project on Altru.dev using the official restrained Ukrainian vyshyvanka identity from this repository.

## Source asset

Repository source:

`assets/brand/altru-browser-mark.svg`

Repository visual foundation commit:

`fe3e197b0a633023356fadf0e1224dd0c89d711e`

## Governed production transaction

Altru Remote transaction:

`f4e5ce7d-152d-48e6-bd7e-859e6d615913`

Transaction state: **committed**.

Production mutations were hash-bound to exact predecessor state:

- `index.php`: `b7f1176f04758c8894b870e3debaa95fe81afe682ea3fdfb33c950e480b7cf79` → `0738275c7679b4c8a1ca4bc243255123a4bf46169535f8983391bc7451a43d73`
- `includes/views.php`: `0c3b16bbdadb9a068c7ac497d839542acbe19b610fae62a7afb933cdf8b1ec49` → `5a1184181b8756dfe45af30509f647cda3df98804680c8a24f6f7a7e42784161`
- `assets/css/site.css`: `40789e4394f99d7193360bfc3c32a5da33639f20884ef64a6ebbcaad557df183` → `db0a5e7ff1bc69ae85452cb03d217388189283642b809ce38e015b0eaeadd0d4`
- `assets/brand/altru-browser-mark.svg`: absent → `2b30e000d9bcddc3531e383033e98e0ae8eb25b268790e6c4994dcbe05bd0fe1`

Transaction commit receipt:

`f639f9f3a51a87671a85f53176ed78ddcf236b3b4fa2a1563d9f6f9124f43b83`

## Live verification

Fresh uncached HTTPS verification confirmed:

- Altru Browser is the first product in the featured project grid;
- card class is `product-altru-browser`;
- card target is `https://github.com/altrudev/Altru-Browser`;
- official mark returns HTTP 200 as `image/svg+xml`;
- production CSS contains the scoped Altru Browser vyshyvanka treatment;
- the featured grid remains eight cards by replacing the former oldest Dweav Trace slot.

## Boundary

The live card describes the browser as an experimental/prototype public project. It does not claim production browser readiness.

Rollback remains hash-bound through the Altru Remote transaction record if a production defect is discovered before later edits supersede the transaction state.
