# Warpeer DMG

Build the current `warpeer` terminal-only product for the host architecture:

```sh
./script/warpeer/build-dmg
```

The script always replaces `build/warpeer/Warpeer.dmg`. The complete
Cargo target directory, staging bundle, and temporary disk image live in an
isolated temporary directory and are removed after every successful or failed
build. `build/` is intentionally ignored by Git. The default mode creates an
ad-hoc signed local artifact.

The installer window uses a native Finder background and positions Warpeer and
Applications in a fixed layout. The disk image does not package background
artwork or a custom volume icon.

For direct distribution, install a Developer ID Application certificate and
authenticate `asc` on the build machine. Run:

```sh
WARPEER_SIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
./script/warpeer/build-dmg --distribution
```

Distribution mode enables hardened runtime, a secure timestamp, Apple
notarization through the ASC CLI, stapling, and Gatekeeper verification. An
Apple Development or Apple Distribution identity is not valid for direct DMG
distribution. The signing private key remains in the macOS Keychain; ASC
credentials remain in the user's ASC configuration and are never copied into
the app bundle or disk image.

The terminal-only profile skips the AI settings schema and generated license
aggregation used by Warp's full release bundle. Source licenses remain in the
checkout; use the upstream release pipeline for a public notarized artifact.

The generated application icon lives under `app/channels/warpeer/icon/`. Run
`./script/warpeer/build-icon` only after replacing its ImageGen-created
`source.png`.
