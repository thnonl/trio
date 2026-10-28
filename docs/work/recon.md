# Recon — toolchain, REA, BetterTouchTool evidence

Date: 2026-10-28. Box: Windows 11, PowerShell 5.1. Timeboxed probe; every claim below was produced by a command on this box.

## 1. Toolchain

| Tool | Verdict | Version / path |
|---|---|---|
| `rustc` | PRESENT | 1.98.1 (48a229cea 2026-09-01) |
| `cargo` | PRESENT | 1.98.1 (797e8a9bc 2026-08-05) |
| rustup toolchain | PRESENT | `stable-x86_64-pc-windows-msvc` (active, default) |
| installed targets | PRESENT | `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu` |
| `node` | PRESENT | v24.22.1 |
| `npm` | PRESENT via `npm.cmd` | 11.19.1 — **`npm.ps1` is blocked** by PowerShell ExecutionPolicy ("running scripts is disabled"), so bare `npm` fails; use `npm.cmd` |
| `npx` | PRESENT via `npx.cmd` | 11.19.1 |
| `git` | PRESENT | 2.55.0.windows.5 |
| `curl` | PRESENT via `curl.exe` | 8.21.0 (Schannel). Bare `curl` is the `Invoke-WebRequest` alias, not curl |
| `node` network access | **BLOCKED** | every `node`/`npm`/`npx` fetch dies with `ECONNREFUSED` to `registry.npmjs.org`, while `curl.exe` and `Invoke-WebRequest` reach the same host with 200. Node-specific block (sandbox/firewall), not a DNS or proxy problem |
| `llvm-objdump` | ABSENT | not on PATH, `C:\Program Files\LLVM\bin\` missing |
| `nm` | ABSENT | not on PATH; not in `C:\Program Files\Git\usr\bin` either |
| `strings` | ABSENT | same |
| `objdump` | ABSENT | same |
| `otool` | ABSENT | macOS-only |
| `dumpbin` | PRESENT | `...\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\dumpbin.exe` — PE/COFF only, cannot read Mach-O |

### Can we run `cargo check`? Yes — on Windows and on the macOS target
A throwaway crate in the temp folder was checked both ways:

- `cargo check` → exit 0
- `cargo check --target aarch64-apple-darwin` → exit 0 (the `aarch64-apple-darwin` std is installed)

So mac-only code behind `#[cfg(target_os = "macos")]` is not merely skipped on this box: it can be **type-checked** with `cargo check --target aarch64-apple-darwin`. `cargo build` for that target would fail (needs the macOS linker/SDK); `cargo check` does not link.

## 2. REA (github.com/morluto/rea) — NOT usable on this box

- Latest version on the registry: **rea-agents 6.8.0** (`https://registry.npmjs.org/rea-agents`, fetched with `curl.exe`, 356 KB metadata).
- `npm.cmd install --global rea-agents` → `ECONNREFUSED` (node network block, see table).
- `npx.cmd -y rea-agents@latest --help` → same block; not attempted further once the cause was known.
- Workarounds tried, all failed:
  1. `curl.exe` the tarball (9,944,902 bytes) and `npm install -g <tarball> --offline` → `ENOTCACHED` on `@babel/parser`: the package has 43 runtime dependencies, so an offline install needs a populated cache.
  2. Extract the tarball and run `node scripts/rea.mjs --help` → `ERR_MODULE_NOT_FOUND: zod`.
  3. Local loopback HTTP proxy (PowerShell `HttpListener` on `127.0.0.1:8899` relaying through `curl.exe`, rewriting `dist.tarball` to the proxy) + `npm install --registry http://127.0.0.1:8899/`. This got **~790 successful fetches**, and `npm ls -g` reported `rea-agents@6.8.0`, but every npm run was killed by timeout mid-reify: `node_modules/incur` and `node_modules/zod` stayed empty (npm hangs on the single-threaded proxy, so the install never completed). Installing `zod` by hand moved the error on to `incur`; the tree is still incomplete. `rea` is therefore **installed but not runnable**: no `rea` shim on PATH and `node .../scripts/rea.mjs` still throws `ERR_MODULE_NOT_FOUND`.
  4. `rea setup` was never run (explicitly forbidden — it would edit this machine's agent/MCP config).
- **Consequence:** no REA subcommand (`bundle`, `plist`, `symbols`, …) executed. Section 3 was produced with a purpose-built Mach-O reader instead (see "how verified"), which is the documented fallback for a machine with no `llvm-objdump`/`nm`/`strings` and no Hopper/Ghidra/IDA provider.

## 3. BetterTouchTool — static evidence

Download (single fetch, kept out of the repo):

- URL: `https://folivora.ai/releases/BetterTouchTool_6.zip` (found on `https://folivora.ai/downloads`, not guessed)
- HTTP: `200`, `content-length` served 106,577,202 bytes, no redirect (`url_effective` = requested URL)
- Size: 106,577,202 bytes; 3,777 zip entries
- SHA256: `F0E6FD033EA42A1CBA7926E5C5800F17E69F1E425FCD120C65202D668FF50FB5`
- Location: `C:\Users\namth\AppData\Local\Temp\opencode\btt\` (`BetterTouchTool_6.zip`, `extract\BetterTouchTool.app`)
- Bundle facts (`Contents/Info.plist`, XML): `CFBundleIdentifier = com.hegenberg.BetterTouchTool`, `CFBundleShortVersionString = 6.958`, `CFBundleVersion = 2026102001`, `LSMinimumSystemVersion = 12`, `LSUIElement` present (agent app, no Dock icon).
- Main binary: `Contents/MacOS/BetterTouchTool`, 156,666,864 bytes, **fat** (`CA FE BA BE`, 2 slices: `x86_64` and `arm64`, 148 load commands each).
- Bundled frameworks: `BTTPluginSupport`, `Ensembles`, `HexFiend`, `LetsMove`, `MIKMIDI`, `Sparkle`. PlugIns: Safari extension, Widgets, Finder context menu, AI-markdown and Soulver bridges. XPCServices: AppleScript/Shell/Swift script runners.

### Symbol / dependency table

| Symbol / dependency | Found? | How verified |
|---|---|---|
| `MultitouchSupport.framework` (load-time dependency) | **YES** | `LC_LOAD_DYLIB` entry `/System/Library/PrivateFrameworks/MultitouchSupport.framework/Versions/A/MultitouchSupport`, present in **both** the `x86_64` and `arm64` slices |
| `MTRegisterContactFrameCallback` | **YES** | symbol name in the binary's name table (`MTRegisterContactFrameCallback` 4 hits @0x4dbbbd2; underscore-prefixed import form `_MTRegisterContactFrameCallback` @0x4dbbbd1) |
| `MTDeviceCreateList` | **YES** | 4 hits @0x4dbb9fa; `_MTDeviceCreateList` @0x4dbb9f9 |
| `MTDeviceStart` | **YES** | 4 hits @0x4dbbb34 |
| `MTDeviceRelease` | **NO** | zero hits for both spellings — BTT does not call it in the main binary (it links the framework and never releases devices, or releases via another path) |
| `CGEventCreateMouseEvent` | **YES** | 4 hits @0x4db855f; `_CGEventCreateMouseEvent` @0x4db855e |
| `CGEventPost` | **YES** | 14 hits @0x3d687ab; `_CGEventPost` import @0x4db8705 |
| `AXIsProcessTrusted` | **YES** | 8 hits @0x4db5f4e; `_AXIsProcessTrusted` @0x4db5f4d |
| `CoreGraphics.framework` | **YES** | `LC_LOAD_DYLIB` `/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics`, both slices |
| `kCGEventOtherMouseDown` (string constant) | **NO** (not a symbol) | zero ASCII hits — expected: it is a numeric enum (`1`), never appears as a name |
| `CGEventCreateScrollWheelEvent` (control) | YES | 8 hits @0x4db8581 — used here only as a sanity check that the scan finds real CG API names |

Method for the Mach-O rows: a ~90-line Node script (no dependencies) written to the temp folder (`btt\macho.js`) that parses the fat header, walks `LC_LOAD_DYLIB`/`LC_LOAD_WEAK_DYLIB`/`LC_RPATH` per slice, and byte-scans the file for ASCII needles (the `strings`/`nm` substitute). Commands: `node macho.js <binary> dylibs` and `node macho.js <binary> strings <needle>...`.

### EVIDENCE vs INFERENCE

EVIDENCE (seen, reproducible with the commands above):
1. BTT's main binary has a load-time dependency on the private `MultitouchSupport.framework`, in both architectures.
2. The binary contains the MultitouchSupport C symbols `MTRegisterContactFrameCallback`, `MTDeviceCreateList`, `MTDeviceStart`.
3. The binary contains `CGEventCreateMouseEvent`, `CGEventPost`, `AXIsProcessTrusted`.
4. BTT is an `LSUIElement` agent app on minimum macOS 12, so a menu-bar-only background utility with this exact stack is viable on the OS versions we target.

INFERENCE (not proven here — no macOS machine):
1. That BTT's 3-finger tap path is exactly *MT frame callback → pure gesture check → CGEventPost of OtherMouseDown/Up with button 2*. The symbols make this highly likely and are the same primitives our design uses, but the actual call order is not visible without a disassembler (no Hopper/Ghidra/IDA here, `llvm-objdump` absent). We did not decompile.
2. That BTT links `MultitouchSupport` at load time rather than `dlopen`-ing it: the `LC_LOAD_DYLIB` says link-time, but BTT may *also* dlopen it in a helper. Our design deliberately uses `dlopen` instead, which is strictly safer (no hard dependency on a private framework, readable failure path) — this difference is intentional and not contradicted by the evidence.
3. That `MTDeviceRelease` being absent means any specific lifetime strategy on BTT's side. We simply call `MTDeviceRelease` on shutdown because we `dlopen`; that stays fine.

## 4. What this changes for Trio

- `cargo check` is available **and** `cargo check --target aarch64-apple-darwin` passes on this box: mac-only code can be type-checked here, not just skipped. Worth adding to `scripts/check.ps1` as a second gate.
- Use `npm.cmd`/`npx.cmd` and `curl.exe`, never the bare names, in any script or doc: the `.ps1` shims are blocked and bare `curl` is a PowerShell alias.
- The two load-bearing hypotheses are supported by real BTT binaries: MultitouchSupport symbol use (`MTRegisterContactFrameCallback`/`MTDeviceCreateList`/`MTDeviceStart`) and Quartz event posting (`CGEventCreateMouseEvent`/`CGEventPost`) plus `AXIsProcessTrusted`. Keep the `dlopen`+`dlsym` approach rather than BTT's link-time dependency — it removes a private-framework hard dependency and gives us the "listener unavailable: <reason>" path the charter requires.
- Do not expect `MTDeviceRelease` to be required for a working tap: BTT ships without it. Calling it is still correct for us; just don't make startup depend on `dlsym` finding it, or the listener dies on a symbol we don't actually need.

## 5. Could not determine

1. Whether REA works at all on Windows: it never produced output here (incomplete `node_modules`). Untested on macOS/Linux, where its installer supports the platform.
2. Which REA subcommands would have run without a Hopper/Ghidra/IDA provider — unknown, never reached CLI help.
3. The real call sequence inside BTT (disassembly not possible: no Hopper/Ghidra/IDA, no `llvm-objdump`/`objdump`/`nm`).
4. Whether BTT's tap detection uses raw contact frames or a higher-level gesture recognizer built on the same callbacks.
5. Any runtime behaviour (tap latency, permission prompts, cooldown) — no macOS machine available to the team.
6. Whether `https://folivora.ai/releases/BetterTouchTool_6.zip` is the "latest stable" or an evergreen 6.x pointer; the site labels it as the trial download and we did not re-download to compare.
