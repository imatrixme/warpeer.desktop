use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let plugin_id = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| anyhow::anyhow!("plugin ID argument is required"))?;
    let package_root = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("package root argument is required"))?;
    warpeer_plugin_worker::run(&plugin_id, &package_root, 16)
}
