#!/usr/bin/env python3
"""Differential gate: native Taffy candidate tracks vs matched-font Chromium.
Not a general native scene conformance proof.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from verify_reference import ProofParser, verify, HERE

ROOT = HERE.parents[1]
FONT = Path(os.environ.get("ALTRU_TEST_FONT", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"))
PROFILE = Path(os.environ.get("ALTRU_CHROMIUM_PROFILE_PARENT", str(Path.home() / "snap/chromium/common")))
TEMP = Path(os.environ.get("ALTRU_REFERENCE_TEMP", str(ROOT / "fixtures/grid-differential")))
TOLERANCE = 1.05   # native Taffy snaps to whole pixels; Chromium reports subpixels

def main():
    font = FONT.read_bytes()
    html = (HERE / "reference.html").read_text()
    face = "@font-face {font-family: AltruProof; src: url(data:font/ttf;base64," + base64.b64encode(font).decode() + ") format('truetype');}"
    html = html.replace("body { margin: 0; font: 16px monospace; }",
                        face + "\nbody { margin: 0; font: 16px AltruProof; }")
    html = html.replace("const cases = [", "document.fonts.ready.then(() => {\nconst cases = [")
    html = html.replace("</script>", "});\n</script>")
    PROFILE.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="altru-h4-profile-", dir=PROFILE) as profile:
        with tempfile.TemporaryDirectory(prefix="altru-h4-html-", dir=TEMP) as tmp:
            page = Path(tmp) / "matched.html"
            page.write_text(html)
            args = [os.environ.get("ALTRU_REFERENCE_BROWSER", "chromium"), "--headless", "--no-sandbox",
                    "--disable-gpu", "--disable-background-networking", "--disable-extensions",
                    "--user-data-dir=" + profile, "--virtual-time-budget=1000", "--dump-dom", page.as_uri()]
            run = subprocess.run(args, capture_output=True, text=True, timeout=35)
            if run.returncode:
                raise RuntimeError("Chromium failed: " + run.stderr[-250:])
    parser = ProofParser()
    parser.feed(run.stdout)
    if len(parser.proofs) != 1:
        raise ValueError("reference output missing")
    reference = verify(json.loads(parser.proofs[0]))
    env = dict(os.environ)
    env["ALTRU_TEST_FONT"] = str(FONT)
    env.setdefault("TMPDIR", str(Path.home() / ".cache/altru-edge-tmp"))
    env.setdefault("CARGO_TARGET_DIR", str(Path.home() / "src/Altru-Browser/target"))
    rust = subprocess.run(["cargo", "test", "--offline", "--features", "taffy-layout",
                           "h4_emit_resolved_font_track_geometry", "--", "--nocapture"],
                          cwd=ROOT, env=env, capture_output=True, text=True, timeout=120)
    if rust.returncode:
        raise RuntimeError("native candidate test failed: " + rust.stdout[-300:])
    matches = re.findall(r"H4_NATIVE (fr-auto|auto-fr|fr-fr) ([0-9.]+) ([0-9.]+) ([0-9.]+) ([0-9.]+)", rust.stdout)
    native = {name: [float(v) for v in vals] for name, *vals in matches}
    if len(native) != 3:
        raise ValueError("missing native geometry cases")
    failures = []
    details = []
    for case in reference:
        expected = [float(v) for child in case["children"] for v in (child["x"], child["width"])]
        actual = native[case["id"]]
        errors = [round(abs(a-b), 4) for a,b in zip(actual, expected)]
        details.append({"case": case["id"], "native": actual, "chromium": expected, "errors": errors})
        if max(errors) > TOLERANCE:
            failures.append(case["id"])
    result = {"status": "CANDIDATE_DIFFERENTIAL_PASS" if not failures else "CANDIDATE_DIFFERENTIAL_FAIL",
              "font_sha256": hashlib.sha256(font).hexdigest(), "tolerance_px": TOLERANCE,
              "details": details, "failures": failures}
    print(json.dumps(result, sort_keys=True))
    if failures:
        sys.exit(1)

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"H4_DIFFERENTIAL_ERROR: {exc}", file=sys.stderr)
        sys.exit(2)
