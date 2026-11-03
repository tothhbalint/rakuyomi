# Handoff notes

> **Notice:** This work was performed by an AI coding agent (DeepSeek V4.1
> Flash) in a single session. The changes compile and several sources were
> exercised manually, but full automated KOReader e2e coverage was not completed.
> Review the diff before releasing.

This document describes the current state of the work to make rakuyomi work with
current KOReader releases and current Aidoku sources. It is intended for whoever
continues the work.

## What was broken

1. **Aidoku source ABI changed completely.** Current sources (aidoku-rs 0.3 /
   Aidoku 0.7+) use a descriptor + postcard ABI. The old rakuyomi implementation
   spoke the legacy "Aidoku value store" ABI.
2. **Source lists changed format.** Current lists are
   `{ "name": "...", "sources": [ { "id": "...", "downloadURL": "..." } ] }`.
   The old code expected a bare array with a `file` key.
3. **Source manifests changed.** New `source.json` files use `languages`
   (array) instead of `lang`, and include other fields that the old struct
   rejected/mishandled.
4. **`settings.json` can contain newer setting types** (`multi-select`, `page`,
   `editable-list`, `login`, ...), which used to make installation fail.
5. **KOReader compatibility:** `testing.lua` redefined `sockaddr_un` through
   `ffi.cdef`, which is a hard error on current LuaJIT/KOReader builds.

## What is implemented

### New source runtime

New module: `backend/shared/src/source/abi_v2/`.

- `source.rs` – `AbiV2Source`, source loading, export invocation, result buffer
  handling, conversion to rakuyomi's internal model.
- `model.rs` – postcard-compatible copies of the current Aidoku structs. **Keep
  field/variant order in sync with aidoku-rs.** Postcard does not encode names.
- `store.rs` – descriptor store, request/response state, defaults.
- `html.rs` / `html_dom.rs` – current HTML ABI on top of `scraper` (some DOM
  mutation operations are still unimplemented, matching the reference test
  runner).
- `net.rs` – request/response ABI, `send_all`, headers, status, URL, HTML
  parsing, timeouts.
- `std.rs`, `env.rs`, `defaults.rs` – std/env/defaults imports. `std.print` and
  `std.abort` were added because newer sources import them.
- `settings.rs` – parses static `settings.json` leniently. Only settings the
  existing Lua UI understands are exposed; defaults for all supported types are
  still made available to sources.
- `mod.rs` – registers all imports. `js` and `canvas` are registered as stubs so
  sources can instantiate; unsupported operations return errors.

`Source::from_aix_file` detects the ABI by checking whether the module exports
`get_search_manga_list`, then dispatches to the new runtime or the legacy
runtime. Legacy sources still work and were tested with a legacy MangaDemon
`.aix`.

Other changes:

- Chapter downloads now use `buffered` instead of `buffer_unordered`, so pages
  are written to the CBZ in reading order even though downloads run
  concurrently. Before this fix pages could be shuffled depending on which
  request finished first.
- `Settings::source_lists` defaults to the current community list plus the Yomu
  list when the field is omitted from `settings.json`. An explicit empty array
  still disables source lists. If `settings.json` does not exist at all, the
  server now creates it with these defaults, so a freshly copied plugin starts
  without manual setup.
- The result buffer reader now subtracts the 8-byte Aidoku header from the
  encoded length instead of reading `len` bytes from the payload.
- `SourceInfo` now accepts `languages: Vec<String>` and optional `lang`.
- Global `languages` setting is exposed to sources as the `languages` default.
  If unset, the source's first declared language is used (English preferred).
- `Page` has `context: Option<HashMap<String, String>>` and `manga_id`; image
  requests receive the page context.
- Source list parsing accepts both old arrays and new `{ sources: [...] }`
  objects, `downloadURL`/`download_url`/`file` fields.
- Duplicate source ids across lists are deduplicated by highest version, and
  installation picks the highest version. This matters when both the community
  list and Yomu list are configured.
- `frontend/rakuyomi.koplugin/testing.lua` wraps `ffi.cdef` in `pcall` so it
  works on current KOReader.

### Added test tool

`backend/shared/examples/probe.rs`:

```sh
cd backend
cargo run -p shared --example probe -- <source.aix> "death note" [page-index]
```

It loads a source, searches, selects "Death Note", chooses chapter 1, prints the
page list, and optionally downloads a page to `/tmp/rakuyomi-probe-<n>.img`.

## Tested sources / results

Tested on 2026-11-03 against current community and Yomu sources.

| Source | Result |
| --- | --- |
| **multi.kagane (Yomu v26)** | **Works well.** Death Note, MANGA Plus scanlation, chapter 1: 47 pages, 1400x2100 WebP. Full chapter downloaded and packed as a 27.9 MB CBZ with correct page order. **Recommended source.** |
| multi.mangafire (v8) | Works. Death Note chapter 1: 1400x2100 JPEG. Needs the `languages` default (see above). |
| en.atsumaru (v1) | Works. Death Note chapter 1: 1066x1600 WebP. |
| en.weebcentral (v8) | Works. Death Note chapter 1: 1066x1600 PNG/JPEG. |
| en.mangabat (v9) | Works. Death Note chapter 1: 692x1100 WebP (low resolution). |
| en.mangago (v4) | Works. Death Note chapter 1: 692x1100 JPEG (low resolution). |
| en.mangademon (v7) | Works. Includes an ad image as page 0; later pages 692x1100. |
| en.mangareadorg (v2) | Works. Death Note chapter 1: 692x1100 JPEG. |
| en.mangadistrict (v1) | Works (search/chapters/pages); mostly one-page chapters in the sample. |
| multi.mangadex (v14) | Search/chapters work; some chapters are externally hosted and return "Missing chapter data". Select another chapter. |
| en.comix (Yomu v138) | **Not usable without a real webview.** The source deliberately requires Comix's browser checks and its renderer to de-scramble pages. The source itself returns error `-1` with the stub JS/webview imports. Implementing this would require a real browser/webview, which is not feasible in the Rust backend on KOReader. |
| en.asurascans, en.mangakakalot, en.manganato | Search returned zero results in this environment (Cloudflare challenge / site blocking). Runtime path is fine; the sites are the problem. |
| en.batcave | Requires interactive verification; not usable headlessly. |
| legacy en.mangademon v5 | Legacy ABI still works (search, chapters, pages). |

Specific results for Kagane Death Note:

- Manga id: `019d35c7-4438-7a1d-9363-a3db847b4db4`
- Chapter 1 id: `019d35c7-497b-7272-8623-a3ad3ff63ae7`
- 47 pages, all 1400x2100 WebP.
- Full download via server endpoint works.

## Server integration checks performed

With a server home containing both source lists:

```json
{
  "source_lists": [
    "https://aidoku-community.github.io/sources/index.min.json",
    "https://smexhy.github.io/yomu-aidoku-sources/index.min.json"
  ],
  "languages": ["en"],
  "storage_size_limit": "2GB"
}
```

- `/available-sources` returned 149 deduplicated sources.
- Installing `multi.kagane` installed version 26 from the Yomu list.
- `/mangas?q=death note` returned Kagane results.
- Add-to-library, refresh-chapters (108 chapters), list chapters, and download
  chapter all worked.
- The downloaded CBZ contains `0000.webp` ... `0046.webp` in order (47 pages,
  27.9 MB) with correct dimensions.
- A `settings.json` with no `source_lists` field also returned all 149 sources,
  proving the new built-in default lists work.

## KOReader checks

- Tested with KOReader nightly `v2026.07.2-306-gb166eb5_2026-11-02_macos`.
- Plugin loads successfully; backend starts.
- `testing.lua` cffi fix was needed for `RAKUYOMI_IS_TESTING=1`.
- Full automated e2e run was not completed; the hand-test path is:
  1. Copy `frontend/rakuyomi.koplugin` into `koreader/plugins/`.
  2. Put the built `server` and `uds_http_request` binaries in the plugin folder.
  3. Create `rakuyomi/settings.json` with the source lists above.
  4. Launch KOReader, open the Rakuyomi menu, install Kagane, search
     "death note", open the first result, refresh chapters, download/open a
     chapter.
  5. The testing hooks use Shift+F9 to open the library and Shift+F8 to dump UI
     when `RAKUYOMI_IS_TESTING=1`.

## Known missing pieces / next steps

1. **Finish e2e verification** with the current KOReader. The e2e Python setup
   now uses the current source lists, but the launch path was not exercised in
   this environment:
   - `e2e-tests` still launches through the devenv `dev` command.
   - `packages/koreader.nix` still pins KOReader 2024.11.
2. **Dynamic settings** (`get_settings` export) are not parsed. Sources with no
   `settings.json` but dynamic settings may lack defaults. MangaFire was fixed
   via the global `languages` default, but a general `get_settings` decoder is a
   good follow-up.
3. **Filters/listings/home** are not surfaced in the UI. Search works; source
   filters and home listings are ignored.
4. **Canvas / page-image processing** is stubbed. `process_page_image` sources
   (e.g. Mangago) may fail for protected pages. Comix also needs this.
5. **HTML DOM mutation** (`set_attr`, `set_text`, etc.) is unimplemented, same as
   the reference aidoku-rs test runner.
6. **Some Yomu sources still fail on this environment.** Kagane and MangaDotNet
   work end-to-end. Atsumaru returns `-1` from `get_page_list` (the source fails
   to deserialize the synthetic `Manga` input), Mangaball reports a missing page
   URL for some chapters, and OniSaga is behind a Cloudflare challenge. These do
   not affect the Kagane Death Note path.
7. **Comix** cannot be fully supported without a webview. The Yomu source README
   explicitly says Comix blocks third-party readers and uses a custom renderer
   for scrambled pages.

## Kindle hardfloat build (done)

The `arm-unknown-linux-musleabihf` build now works with Zig 0.17. Two details
were required on Linux:

- Zig 0.17 rejects the `--target=arm-unknown-linux-gnueabihf` flag that the
  `cc` crate passes, so the wrapper scripts strip `--target`/`-target` and force
  `-target arm-linux-musleabihf`.
- Rust's self-contained CRT objects collide with Zig's own `crt1.o`, so the
  build uses `-C link-self-contained=no`.

```sh
rustup target add arm-unknown-linux-musleabihf

cat > /tmp/zig-cc-armhf <<'EOF'
#!/usr/bin/env bash
args=(); skip=0
for arg in "$@"; do
  if [ "$skip" = 1 ]; then skip=0; continue; fi
  case "$arg" in
    --target=*) continue ;;
    -target) skip=1; continue ;;
  esac
  args+=("$arg")
done
exec /tmp/zig/zig cc -target arm-linux-musleabihf "${args[@]}"
EOF
# Same for /tmp/zig-cxx-armhf with `zig c++`.

export CC_arm_unknown_linux_musleabihf=/tmp/zig-cc-armhf
export CXX_arm_unknown_linux_musleabihf=/tmp/zig-cxx-armhf
export AR_arm_unknown_linux_musleabihf="/tmp/zig/zig ar"
export CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABIHF_LINKER=/tmp/zig-cc-armhf
export CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABIHF_RUSTFLAGS="-C target-feature=+crt-static -C link-self-contained=no"

cd backend
cargo build --release --target arm-unknown-linux-musleabihf \
  -p server -p uds_http_request
```

The resulting binaries are static ARM EABI5 hard-float executables. The packaged
artifact is `build/rakuyomi-kindlehf.zip` (plugin Lua files plus `server`,
`uds_http_request`, and `BUILD_INFO.json`); `build/rakuyomi.koplugin/` contains
the unpacked folder. `build/` is gitignored.

## Warnings / cleanup

- `cargo check --workspace` passes.
- Warnings were cleaned from the new ABI code at the time of handoff.
- `backend/shared/examples/probe.rs` uses `env_logger` as a dev-dependency.
- `scraper` was bumped to 0.25 with the `atomic` feature. `HtmlDocument`,
  `HtmlNode`, etc. have unsafe `Send`/`Sync` impls because `scraper` uses
  non-atomic internals; access is serialized behind the source mutex.
