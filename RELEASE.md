# Building release installers

Proscenium ships as platform installers built by the Tauri bundler:

| Platform | Artifacts | Toolchain |
|----------|-----------|-----------|
| Windows  | `.msi` (WiX) + `-setup.exe` (NSIS) | downloaded automatically by Tauri |
| macOS    | `.app` + `.dmg` | Xcode Command Line Tools |

> **There is no cross-compilation.** A Windows installer must be built on Windows
> and a macOS installer must be built on macOS. To produce both from one push, use
> CI with a Windows runner and a macOS runner — see [Building both in CI](#building-both-in-ci).

Everything below runs `npm run tauri build`, which:
1. runs `npm run build` (type-check + Vite production bundle into `dist/`),
2. compiles the Rust backend in `--release` with the `custom-protocol` feature,
3. bundles the platform installers and **signs the auto-update artifacts**.

Outputs land in `src-tauri/target/release/bundle/`.

---

## One-time setup: the updater signing key

`bundle.createUpdaterArtifacts` is enabled, so **every** release build must have the
updater signing key in the environment or it fails. The same key is used on both
platforms (the signature is what the installed app verifies before applying an update).

The key already exists at `src-tauri/proscenium-updater.key` (gitignored). Its public
half is committed as `plugins.updater.pubkey` in `tauri.conf.json`. To regenerate the
pair (invalidates updates for already-installed apps):

```sh
npx tauri signer generate --ci -p "" -w src-tauri/proscenium-updater.key -f
# then copy the printed public key into plugins.updater.pubkey in tauri.conf.json
```

Before each build, export the key (commands shown per-platform below). Keep the private
key and its password (empty here) out of version control and in CI secrets only.

---

## Windows (`.msi` + `.exe`)

Prerequisites: the repo's normal dev setup (Rust GNU toolchain, Node via fnm — see
[DEVELOPMENT.md](DEVELOPMENT.md)) plus **both bundled DLLs staged in `src-tauri/lib/`**:

```powershell
# Player engine (from your mpv-winbuild) + the WebView2 loader shim
# WebView2Loader.dll: the GNU/MinGW build links it dynamically (MSVC would static-link
# it), so it MUST ship next to the exe or the app dies on launch with
# "WebView2Loader.dll was not found". Tauri's NSIS template does not add it for you.
Copy-Item "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-*\x64\WebView2Loader.dll" src-tauri\lib\
# (libmpv-2.dll should already be in src-tauri\lib\ from dev setup)
```

WiX and NSIS are downloaded by Tauri on first build; the WebView2 *runtime* (separate
from the loader) is installed at runtime by the download bootstrapper.

```powershell
# Make sure Node is on PATH (fnm-managed)
$env:PATH = "$env:APPDATA\fnm\node-versions\v22.16.0\installation;$env:PATH"

# Updater signing key
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content src-tauri\proscenium-updater.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

npm run tauri build
```

Produces (for version `1.0.0`):

```
src-tauri/target/release/bundle/
├── msi/  Proscenium_1.0.0_x64_en-US.msi   + .msi.sig
└── nsis/ Proscenium_1.0.0_x64-setup.exe   + -setup.exe.sig
```

`libmpv-2.dll` **and `WebView2Loader.dll`** are bundled next to the installed `.exe` via
the `bundle.resources` mapping in `tauri.windows.conf.json`, so the installed app needs
no manual DLL copy. The `.sig` files are the minisign signatures the auto-updater
verifies.

> **Sanity check after building:** confirm both DLLs are actually in the installer —
> `grep -i webview2loader src-tauri/target/release/nsis/x64/installer.nsi` should show a
> `File ... WebView2Loader.dll` line. The cleanest real test is to install on a **fresh
> Windows machine or a new user account** that has never had the dev DLLs lying around.

**Code signing (production):** to avoid SmartScreen warnings, sign the installers with
an Authenticode certificate. Set `bundle.windows.certificateThumbprint` (and
`digestAlgorithm`/`timestampUrl`) in `tauri.conf.json`, or sign the artifacts with
`signtool` after the build. This is separate from the updater signing above.

---

## macOS (`.app` + `.dmg`)

> ✅ **Verified on an Apple Silicon Mac (M1 Pro).** `npm run tauri build` produces a
> self-contained `Proscenium.app` + `Proscenium_1.0.0_aarch64.dmg` with the full libmpv
> dependency tree (47 dylibs) embedded. Still outstanding for *distribution*: Developer-ID
> code signing/notarization (§3 below) and the real updater key. Note also that the native
> video window is currently Windows-only (`ensure_video_host` returns `Ok(None)` on macOS),
> so playback won't render video yet — that's a feature gap, not a build-pipeline one.

Prerequisites:
- Xcode Command Line Tools: `xcode-select --install`
- Node 22 + Rust (`rustup`), same as any platform. The committed `rust-toolchain.toml`
  pins `channel = "stable"`, which resolves to `aarch64-apple-darwin` automatically — no
  per-machine setup on macOS.
- libmpv and its runtime dependencies: `brew install mpv`
- `dylibbundler` to gather libmpv's transitive deps: `brew install dylibbundler`

### 1. Stage libmpv for bundling

The built-in player loads `libmpv.2.dylib` at runtime. For a distributable `.app`, the
dylib **and its transitive dependencies** must be embedded (libmpv pulls in ffmpeg and
others). Copy the Homebrew dylib into `src-tauri/lib/` so the bundler can pick it up:

```sh
cp "$(brew --prefix)/lib/libmpv.2.dylib" src-tauri/lib/libmpv.2.dylib
```

`tauri.macos.conf.json` declares it under `bundle.macOS.frameworks`, which embeds it in
`Proscenium.app/Contents/Frameworks/` and rewrites its install name. `open_libmpv()`
(in `mpv/player.rs`) looks in `../Frameworks` relative to the executable in addition to
next-to-the-binary, so it resolves there.

**Transitive deps (required — the single dylib is not enough):** Homebrew's
`libmpv.2.dylib` links against ~20 other Homebrew dylibs (ffmpeg, libass, libplacebo,
…). `bundle.macOS.frameworks` embeds only the files you list, so without gathering the
tree the `.app` runs *only* on a machine that already has `brew install mpv` — on a
clean Mac libmpv fails to load. Gather and rpath-fix the full tree with
[`dylibbundler`](https://github.com/auriamg/macdylibbundler):

```sh
chmod u+w src-tauri/lib/libmpv.2.dylib   # the staged copy is read-only
dylibbundler -of -b -x src-tauri/lib/libmpv.2.dylib -d src-tauri/lib/ -p @rpath
# dylibbundler does NOT rewrite the -x file's own install-name ID, so fix it too:
install_name_tool -id @rpath/libmpv.2.dylib src-tauri/lib/libmpv.2.dylib
codesign --force -s - src-tauri/lib/libmpv.2.dylib
```

> **CRITICAL — strip the bogus `@rpath/` LC_RPATH (else the app dies with
> "Could not load libmpv").** `dylibbundler -p @rpath` adds a malformed
> `LC_RPATH @rpath/` to the dylibs it rewrites, and on `libmpv.2.dylib` it adds
> it **twice**. dyld refuses to load *any* image with a duplicate `LC_RPATH`, so
> `open_libmpv` fails on every Mac even though the dylib and all 47 deps are
> present. The dylibs do not need their own rpath — the app binary's
> `@executable_path/../Frameworks` resolves their `@rpath/...` deps — so delete
> every `@rpath/` rpath and re-sign before regenerating the framework list:
>
> ```sh
> chmod u+w src-tauri/lib/*.dylib
> for f in src-tauri/lib/*.dylib; do
>   while otool -l "$f" | grep -A2 'cmd LC_RPATH' | grep -q 'path @rpath/'; do
>     install_name_tool -delete_rpath '@rpath/' "$f"
>   done
>   codesign --force -s - "$f"
> done
> # sanity: this must print 0
> for f in src-tauri/lib/*.dylib; do otool -l "$f" | grep -A2 LC_RPATH | grep -c 'path @rpath/'; done | paste -sd+ | bc
> ```

On Apple Silicon this currently gathers **47 dylibs** into `src-tauri/lib/`. Every one
must be listed under `bundle.macOS.frameworks` in `tauri.macos.conf.json`; the binary's
baked-in `@executable_path/../Frameworks` rpath then resolves them. Regenerate the list
straight from the directory so it can't drift:

```sh
python3 -c 'import json,os; d=sorted(f for f in os.listdir("src-tauri/lib") if f.endswith(".dylib")); \
print(json.dumps({"$schema":"https://schema.tauri.app/config/2","bundle":{"macOS":{"frameworks":["lib/"+f for f in d]}}}, indent=2))' \
> src-tauri/tauri.macos.conf.json
```

Verify the resulting `.app` is self-contained (should print `0`):

```sh
cd src-tauri/target/release/bundle/macos/Proscenium.app/Contents/Frameworks
for f in *.dylib; do otool -L "$f" | tail -n +2 | grep -c /opt/homebrew; done | paste -sd+ | bc
```

This is also the LGPL dynamic-linking compliance path for libmpv.

### 2. Build

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat src-tauri/proscenium-updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""

npm run tauri build
```

Produces:

```
src-tauri/target/release/bundle/
├── macos/ Proscenium.app
└── dmg/   Proscenium_1.0.0_aarch64.dmg   (or x64 on Intel)   + .sig
```

`bundle.macOS.minimumSystemVersion` is `11.0` (Big Sur), matching the spec.

### 3. Code signing & notarization (required for Gatekeeper)

An unsigned `.app` triggers a Gatekeeper block on other Macs. For distribution you need
an Apple Developer ID. Provide these to the build via environment variables and Tauri
signs and notarizes automatically:

```sh
export APPLE_CERTIFICATE="…base64 of the .p12…"
export APPLE_CERTIFICATE_PASSWORD="…"
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="…app-specific password…"
export APPLE_TEAM_ID="TEAMID"
npm run tauri build
```

Without these the `.dmg` still builds, but recipients must right-click → Open to bypass
Gatekeeper once.

---

## Auto-update feed

`plugins.updater.endpoints` in `tauri.conf.json` currently points at a **placeholder**
host (`https://releases.proscenium.app/...`). To actually serve updates:

1. Host a static updater manifest (one per `{{target}}/{{arch}}/{{current_version}}`)
   that returns the new version, notes, and the signed installer URL.
2. The `version` in the manifest must be higher than the running app's `version` in
   `tauri.conf.json` for the launch-time check (`src/lib/updater.ts`) to offer it.
3. Attach the `.sig` content as the `signature` field; the app verifies it against the
   committed `pubkey` before installing.

See the Tauri updater docs for the exact manifest schema.

---

## Building both in CI

The clean way to produce Windows **and** macOS installers from a single tag is GitHub
Actions with a matrix of runners and the official action. Sketch:

```yaml
name: release
on:
  push:
    tags: ["v*"]
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: windows-latest
          - os: macos-latest      # Apple Silicon runner
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - uses: dtolnay/rust-toolchain@stable
      - run: npm ci
      # Windows: drop libmpv-2.dll into src-tauri/lib/ (download or restore from cache)
      # macOS:   brew install mpv && stage libmpv.2.dylib (+ dylibbundler) into src-tauri/lib/
      - uses: tauri-apps/tauri-action@v0
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # macOS signing secrets (APPLE_*) as above
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Proscenium ${{ github.ref_name }}"
```

`tauri-action` runs the same `tauri build` on each runner and uploads every installer
(and its `.sig`) to a GitHub Release, which can double as the updater feed.
