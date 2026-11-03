> [!NOTE]
> This fork brings rakuyomi up to date with the current Aidoku source format
> (Aidoku 0.7+ / aidoku-rs 0.3) and the current source lists. The original
> project is no longer actively maintained; [tachibana-shin's
> fork](https://github.com/tachibana-shin/rakuyomi) is another alternative.

# rakuyomi

**rakuyomi** is a manga reader plugin for [KOReader](https://github.com/koreader/koreader).

<p align="center">
    <img src="docs/src/images/demo.gif" width="60%" />
    <br/>
    <em><small><a href="https://seotch.wordpress.com/ubunchu/">"Ubunchu!"</a> by Hiroshi Seo is licensed under <a href="https://creativecommons.org/licenses/by-nc/3.0/">CC BY-NC 3.0</a>.</small></em>
</p>

## Features

- Works with current Aidoku sources using the postcard-based descriptor ABI,
  while keeping the legacy runtime for older `.aix` files.
- Reads the current source list format and ships with sensible defaults:
  - [Aidoku Community Sources](https://aidoku-community.github.io/sources/index.min.json)
  - [Yomu](https://smexhy.github.io/yomu-aidoku-sources/index.min.json)
- Searches, reads, and downloads chapters, including page context and image
  request headers supplied by newer sources.
- Writes downloaded chapters as CBZ files with pages in reading order.
- Automatically creates `settings.json` on first launch, so no manual setup is
  required to start browsing sources.

## Installation & Usage

For detailed installation and usage instructions, please check out the
[Installation](https://hanatsumi.github.io/rakuyomi/user-guide/installation/)
and [Quickstart](https://hanatsumi.github.io/rakuyomi/user-guide/quickstart)
sections in the user guide.

In short:

1. Copy the `rakuyomi.koplugin` folder into your KOReader `plugins` folder.
2. Restart KOReader and open the **Rakuyomi** menu.
3. Install a source, search for a manga, and start reading.

The plugin automatically creates `koreader/rakuyomi/settings.json` with the
default source lists on first launch. To customize it, use a configuration like
this:

```json
{
  "source_lists": [
    "https://aidoku-community.github.io/sources/index.min.json",
    "https://smexhy.github.io/yomu-aidoku-sources/index.min.json"
  ],
  "languages": ["en"]
}
```

Omit `source_lists` to use the defaults, or set it to `[]` to disable them.

### Kindle (hard float)

The release archive `rakuyomi-kindlehf.zip` contains static ARM EABI5
hard-float builds of the backend (`server` and `uds_http_request`) together with
the plugin. Extract it and copy `rakuyomi.koplugin` into
`koreader/plugins/` on the device's user partition.

## Development

Build the backend natively with:

```sh
cd backend
cargo build --release -p server -p uds_http_request
```

The Kindle hard-float cross-build requires Zig and the
`arm-unknown-linux-musleabihf` Rust target. Exact commands and current test
results are documented in [`HANDOFF.md`](./HANDOFF.md).

## Contributing

For information on how to contribute to rakuyomi, please check out the
[Setting up the Environment](https://hanatsumi.github.io/rakuyomi/contributing/setting-up-the-environment.html)
section in the guide.
