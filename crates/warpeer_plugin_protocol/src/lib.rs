mod container;
mod framing;
mod ids;
mod manifest;
mod message;
mod surface;

pub use container::*;
pub use ids::*;
pub use manifest::*;
pub use message::*;
pub use surface::*;

pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion(1);
pub const CURRENT_RUNTIME_ABI: RuntimeAbi = RuntimeAbi { major: 1, minor: 2 };

#[cfg(test)]
mod tests;
pub use framing::*;
