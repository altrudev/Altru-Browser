#!/usr/bin/env python3
"""Independent Chromium geometry reference gate; does NOT validate Altru geometry."""
import hashlib
import html.parser
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
IDS = ("fr-auto", "auto-fr", "fr-fr")

class ProofParser(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.inside = False
        self.proofs = []
    def handle_starttag(self, tag, attrs):
        if tag == "pre" and dict(attrs).get("id") == "grid-proof":
            self.inside = True
    def handle_endtag(self, tag):
        if tag == "pre":
            self.inside = False
    def handle_data(self, data):
        if self.inside:
            self.proofs.append(data)

def verify(cases):
    if not isinstance(cases, list) or len(cases) != 3:
        raise ValueError("exactly three cases required")
    if [case.get("id") for case in cases] != list(IDS):
        raise ValueError("case ordering or identifiers changed")
    for case in cases:
        width = case["width"]
        children = case["children"]
        if not isinstance(children, list) or len(children) != 2:
            raise ValueError("expected exactly two children")
        if not isinstance(case["template"], str) or not case["template"]:
            raise ValueError("missing computed grid template")
        values = [width] + [v for child in children for v in (child["x"], child["width"], child["height"])]
        if any(not isinstance(v, (int, float)) or isinstance(v, bool) or not math.isfinite(v) for v in values):
            raise ValueError("nonfinite or nonnumeric geometry")
        if abs(width - 300.0) > .1 or any(child["width"] <= 0 or child["height"] <= 0 for child in children):
            raise ValueError("invalid fixture dimensions")
        left, right = children
        if abs(left["x"]) > .1 or abs(right["x"] - left["width"] - 10.0) > .1:
            raise ValueError("gap/offset mismatch")
        if abs(right["x"] + right["width"] - width) > .1:
            raise ValueError("children do not fill fixed container")
    widths = [[child["width"] for child in case["children"]] for case in cases]
    if abs(widths[2][0] - widths[2][1]) > .1:
        raise ValueError("fractional reference not equal")
    if abs(widths[0][0] - widths[2][0]) < 1:
        raise ValueError("1fr auto reference does not distinguish 1fr 1fr")
    if abs(widths[1][0] - widths[2][0]) < 1:
        raise ValueError("auto 1fr reference does not distinguish 1fr 1fr")
    return cases

def main():
    fixture = HERE / "reference.html"
    browser = os.environ.get("ALTRU_REFERENCE_BROWSER", "chromium")
    profile_parent = Path(os.environ.get("ALTRU_CHROMIUM_PROFILE_PARENT", str(Path.home() / "snap/chromium/common")))
    profile_parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="altru-h4-", dir=profile_parent) as profile:
        command = [browser, "--headless", "--no-sandbox", "--disable-gpu",
                   "--disable-extensions", "--disable-background-networking",
                   "--user-data-dir=" + profile, "--virtual-time-budget=500",
                   "--dump-dom", fixture.as_uri()]
        run = subprocess.run(command, capture_output=True, text=True, timeout=30, check=False)
    if run.returncode:
        raise RuntimeError(f"reference browser exit {run.returncode}: {run.stderr[-400:]}")
    parser = ProofParser()
    parser.feed(run.stdout)
    if len(parser.proofs) != 1:
        raise ValueError("missing or duplicated proof element")
    cases = verify(json.loads(parser.proofs[0]))
    output = {"status": "REFERENCE_ONLY_NOT_ALTRU_CONFORMANCE",
              "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
              "cases": cases}
    print(json.dumps(output, sort_keys=True, separators=(",", ":")))

if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, subprocess.TimeoutExpired, OSError, KeyError, TypeError) as exc:
        print(f"H4_REFERENCE_FAIL: {exc}", file=sys.stderr)
        sys.exit(1)
