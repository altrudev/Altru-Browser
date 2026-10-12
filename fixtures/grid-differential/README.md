# Independent H4 Grid Geometry Reference (not a promotion proof)

Run `chromium --headless --no-sandbox --disable-gpu --dump-dom file://$PWD/fixtures/grid-differential/reference.html` from the repository root on an isolated test host. Extract the JSON from the `grid-proof` element and record browser version, font resolution, viewport, environment and the exact candidate commit. The fixture compares `1fr auto`, `auto 1fr` and `1fr 1fr` with unequal text. The geometry is a reference observation, not a claim that the candidate solver meets the CSS Grid specification. Do not promote H3/H4/H5/H6 from this fixture alone.

Run Chromium reference gate: `python3 fixtures/grid-differential/verify_reference.py` (JSON to stdout).
Run negative validation: `python3 -m unittest discover -s fixtures/grid-differential -p test_verify_reference.py -v`.
This verifies Chromium reference consistency only, not Altru equivalence, live CNET, or H3 integration.
