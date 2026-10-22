#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::AppId;
use warp_core::channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig};

fn main() -> Result<()> {
    ChannelState::set(ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("com", "matrixme", "warpeer"),
            home_config_dir_name: Some(".warpeer".into()),
            use_system_cache_dir: true,
            logfile_name: "warpeer.log".into(),
            server_config: WarpServerConfig::disabled(),
            oz_config: OzConfig::disabled(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    ));

    warp::run_terminal_only()
}

#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>Warpeer</string>
    <key>CFBundleExecutable</key>
    <string>warpeer</string>
    <key>CFBundleIdentifier</key>
    <string>com.matrixme.warpeer</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Warpeer</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array><dict><key>CFBundleURLName</key><string>Warpeer</string><key>CFBundleURLSchemes</key><array><string>warpeer</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright 2026 Warpeer contributors</string>
    </dict>
    </plist>
"#.as_bytes());
