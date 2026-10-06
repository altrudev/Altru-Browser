# VPS Verification Profile

The VPS is the authoritative Phase 1 verification environment.

## Why this profile exists

A full Servo + SpiderMonkey build is substantially heavier than the Light Plane harness and previously starved the VPS control channel. DDC/Frequency therefore treats unconstrained Servo builds as an unsafe operational pattern on this node.

The verification profile uses:

- a user-local dependency sysroot;
- no global package installation;
- no Servo source modifications;
- one Cargo build job for Servo;
- low CPU scheduling priority for the heavy compile;
- persistent evidence logs;
- exact Git commit and toolchain capture.

## Bootstrap

```bash
scripts/bootstrap-servo-vps.sh
```

The bootstrap downloads Ubuntu development packages with `apt-get download` and extracts them beneath `$HOME/src/awef-sysroot`. It does not install them system-wide.

It also exposes the already-installed `/usr/bin/llvm-objdump-21` as a user-local `llvm-objdump`, which SpiderMonkey expects.

## Verification

```bash
scripts/verify-phase1-vps.sh
```

The Servo compile is deliberately constrained with:

```text
CARGO_BUILD_JOBS=1
CARGO_INCREMENTAL=0
nice -n 10
```

This is part of the assurance boundary, not merely a convenience.

## Frequency gate

A verification run is complete only when:

1. formatting passes;
2. Clippy passes with warnings denied;
3. debug tests pass;
4. release tests pass;
5. benchmark completes;
6. Servo compile linkage completes;
7. evidence files contain the exact commit/toolchain hashes.

A control-channel outage during the Servo build is an operational failure signal and does not count as PASS.
