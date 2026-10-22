use warpeer_plugin_protocol::PluginId;

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("unknown plugin: {0}")]
    UnknownPlugin(PluginId),
    #[error("plugin worker failed to launch: {0}")]
    Launch(String),
    #[error("plugin worker is in a crash loop: {0}")]
    CrashLoop(PluginId),
    #[error("plugin has reached its active surface limit")]
    SurfaceLimit,
    #[error("plugin surface bounds are invalid")]
    InvalidSurfaceBounds,
    #[error("plugin surface has {actual} pixels; maximum is {maximum}")]
    SurfacePixels { actual: u64, maximum: u64 },
    #[error("plugin has exceeded its frame-rate budget")]
    FrameRateLimit,
    #[error("plugin worker could not be stopped: {0}")]
    Stop(String),
}
