# Native Platform Verification Matrix

AWEF separates **architectural target**, **compile verification**, and **runtime verification**.

A platform is never promoted because another operating system passed.

| Platform | Representative Rust target | Current state | Required runtime evidence |
| --- | --- | --- | --- |
| Linux | `x86_64-unknown-linux-gnu` | Runtime verified for N1 | native probe + fixtures |
| Android | `aarch64-linux-android` | Compile verified for N1 | Android host adapter + device/emulator runtime probe |
| Windows | `x86_64-pc-windows-gnu` or MSVC on Windows | Planned | native Windows build + UI/renderer probe |
| macOS | `aarch64-apple-darwin` | Planned | native macOS build + Metal/window/accessibility probe |
| iOS/iPadOS | `aarch64-apple-ios` | Planned/policy-gated | entitled host + runtime probe where permitted |

## Compile gate

```bash
scripts/check-native-targets.sh
```

The script checks every installed target with warnings denied and reports missing target standard libraries as `PENDING`, not failure and not success.

## Runtime gate

Compile success does not promote a platform to runtime verified.

Runtime evidence must include:

- native engine version;
- OS / architecture;
- fixture manifest hash;
- deterministic native artifact hash;
- platform-host adapter identity;
- renderer backend identity;
- crash/failure result;
- Frequency promotion state.

## Platform boundary

The AWEF semantic core remains platform neutral.

Concrete platform adapters will be separate modules/crates for:

- window/surface creation;
- GPU backend;
- font discovery;
- clipboard;
- accessibility;
- filesystem/storage;
- networking integration;
- secure secret storage;
- sandbox/process isolation.

No platform adapter is allowed to redefine DOM, CSS, layout, navigation, script-host or security semantics.
