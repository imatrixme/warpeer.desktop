# Warpeer

Warpeer is a terminal-only desktop build derived from the open-source
[Warp](https://github.com/warpdotdev/warp) client. It keeps Warp's fast GPU-rendered terminal
and removes accounts, cloud sync, hosted AI, and other hosted-service features.

## Features

- GPU-accelerated terminal rendering with IME support
- Tabs, vertical tabs, split panes, and pane layout controls
- Command history, completions, global search, and code review
- Bundled Ghostty color themes and configurable terminal motion effects
- Tab privacy with biometric unlock and pane previews
- Worktree agent control center for local CLI agents
- Sandboxed plugin surfaces (PocketJS)

Warpeer never sends telemetry or crash reports, does not auto-update, does not run a local HTTP
server, and does not require sign-in. It stores its data in its own `.warpeer` and application
support directories, separate from Warp.

## Download

Get the notarized macOS disk image from
[Releases](https://github.com/imatrixme/warpeer.desktop/releases).

## Build from source

Warpeer is currently developed and tested on macOS.

```sh
./script/bootstrap --skip-common-skills
./script/run-warpeer            # debug build
./script/warpeer/build-dmg      # ad-hoc signed DMG in build/warpeer/
```

Before sending changes, check the product profile:

```sh
./script/verify-warpeer-profile
```

## License

Warpeer is a modified version of Warp. The `warpui_core` and `warpui` crates are licensed under
the [MIT license](LICENSE-MIT). All other code is licensed under the
[GNU Affero General Public License v3.0](LICENSE-AGPL). See [NOTICE](NOTICE) for attribution
details.

Warpeer is an independent project. It is not affiliated with, endorsed by, or supported by
Warp (Denver Technologies, Inc.). "Warp" is a trademark of its respective owner.
