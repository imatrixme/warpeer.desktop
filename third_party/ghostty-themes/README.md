# Ghostty Theme Snapshot

Warpeer vendors a normalized snapshot of the color themes selected by Ghostty. The application
build is offline and never updates this snapshot automatically.

Run `./script/warpeer/sync-ghostty-themes --ghostty-ref <commit>` to explicitly replace the
snapshot. The command records the Ghostty commit, archive URL, archive checksum, collection
license, generated catalog checksum, and theme count in `manifest.json`.

Only terminal color fields are imported. Other Ghostty configuration options are rejected.
