use warpeer_plugin_protocol::{
    BridgePayload, CapabilityResponse, PluginId, ProtocolLimits, RequestId,
};

use super::WorkerEngine;

#[test]
fn replacing_preview_images_keeps_one_resident_texture() {
    let mut engine = WorkerEngine::new(
        PluginId::parse("dev.warpeer.image-preview").unwrap(),
        b"globalThis.frame = function() {};".to_vec(),
        b"pak".to_vec(),
        ProtocolLimits::default(),
        16,
    )
    .unwrap();

    for index in 0..6 {
        engine
            .materialize_payload(CapabilityResponse {
                request_id: RequestId::new(),
                result: Ok(BridgePayload::RgbaImage {
                    media_type: "image/png".into(),
                    width: 560,
                    height: 420,
                    pixels: vec![index; 560 * 420 * 4],
                }),
            })
            .unwrap();
    }

    assert!(engine.texture.is_some());
}
