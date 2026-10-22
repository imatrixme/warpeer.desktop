#ifdef TERMINAL_VISUALS
struct TerminalGlyphTransform {
    float2 offset;
    float2 scale;
    float alpha;
    float glow;
    float progress;
};
struct TerminalOverlayFragmentData {
    float4 position [[position]];
    float2 local_position;
    float4 color;
    uint kind [[flat]];
    uint effect_kind [[flat]];
    float progress;
    float seed;
};
float terminal_hash(float2 value) {
    return fract(sin(dot(value, float2(12.9898, 78.233))) * 43758.5453);
}
float terminal_ease_out(float value) {
    float inverse = 1.0 - saturate(value);
    return 1.0 - inverse * inverse * inverse;
}
float terminal_spring(float value) {
    value = saturate(value);
    return 1.0 - exp(-8.0 * value) * cos(12.0 * value);
}
TerminalGlyphTransform terminal_glyph_transform(
        const device TerminalPerGlyphUniforms *glyph,
        constant TerminalLayerUniforms *layer)
{
    bool direct_input = (glyph->flags & 1u) != 0u;
    bool erase_ghost = (glyph->flags & 2u) != 0u;
    float duration = max(layer->duration_seconds, 0.001);
    float delay = 0.0;
    uint category = glyph->effect_kind / 100;
    uint variant = glyph->effect_kind % 100;
    if ((category == 1 || category == 2) && variant == 2) {
        delay = glyph->seed * duration * 0.22;
    }
    float progress = saturate((layer->elapsed_seconds - glyph->start_seconds - delay) / duration);
    float eased = terminal_ease_out(progress);
    float spring = terminal_spring(progress);
    TerminalGlyphTransform result;
    result.offset = glyph->previous_cell_origin - glyph->current_cell_origin;
    result.offset *= 1.0 - eased;
    result.scale = float2(1.0);
    result.alpha = 1.0;
    result.glow = 0.0;
    result.progress = progress;
    if (glyph->effect_kind == 0) {
        result.offset = float2(0.0);
        return result;
    }
    if (direct_input) {
        float input_progress = smoothstep(0.0, 1.0, progress);
        float flip = sin(input_progress * M_PI_F) * 0.025;
        result.scale = float2(0.985 + input_progress * 0.015,
            0.84 + input_progress * 0.16 + flip);
        float bottom_pivot = (1.0 - result.scale.y) * layer->cell_size.y * 0.47;
        float staggered_lift = (1.0 - input_progress) * (1.3 + glyph->seed * 1.1);
        result.offset = float2(0.0, bottom_pivot + staggered_lift);
        result.alpha = smoothstep(0.0, 0.15, input_progress);
        result.glow = 0.0;
        return result;
    }
    if (erase_ghost) {
        result.offset = float2(0.0);
        result.alpha = 1.0;
        result.scale = float2(1.0);
        result.glow = 0.0;
        return result;
    }
    if (category == 1) {
        if (variant == 1 || variant == 6) result.offset.y += (1.0 - eased) * 9.0;
        if (variant == 3) result.offset.y = round(result.offset.y / 4.0) * 4.0;
        if (variant == 4) result.glow = sin(progress * M_PI_F) * 0.34;
        if (variant == 5) result.scale = float2(0.96 + eased * 0.04, 0.92 + eased * 0.08);
        if (variant == 6) result.offset.x += (1.0 - eased) * 12.0;
    } else if (category == 2) {
        if (variant == 1) result.offset.y += (1.0 - eased) * -6.0;
        if (variant == 2) result.offset += float2((1.0 - eased) * 9.0, -sin(progress * M_PI_F) * 5.0);
        if (variant == 3) result.scale.x = 0.92 + eased * 0.08;
        if (variant == 4) result.glow = sin(progress * M_PI_F) * 0.28;
        if (variant == 5) result.offset.x += (1.0 - eased) * 18.0;
        if (variant == 6) result.offset.x = round(result.offset.x / 3.0) * 3.0;
    } else if (category == 3) {
        if (variant == 1) result.alpha = eased;
        if (variant == 2) result.offset.y += (1.0 - eased) * 6.0;
        if (variant == 3) result.scale = float2(0.82 + spring * 0.18);
        if (variant == 4) {
            result.scale = float2(0.88 + spring * 0.12, 0.72 + spring * 0.28);
            result.glow = sin(progress * M_PI_F) * 0.2;
        }
        if (variant == 5) result.glow = (1.0 - progress) * 0.4;
        if (variant == 6) result.offset.x += sin(progress * M_PI_F * 2.0) * (1.0 - progress) * 2.5;
        if (variant == 7) result.offset.y -= (1.0 - eased) * 7.0;
        if (variant == 8) result.scale.x = 0.65 + eased * 0.35;
        if (variant == 9) result.scale.y = 0.68 + spring * 0.32;
    } else if (category == 6) {
        result.alpha = eased;
        result.scale.y = max(0.04, eased);
    }

    result.alpha *= smoothstep(0.0, 0.08, progress);
    return result;
}
vertex GlyphFragmentData terminal_glyph_vertex_shader(
        uint vertex_id [[vertex_id]],
        uint instance_id [[instance_id]],
        constant vector_float2 *vertices [[buffer(0)]],
        const device TerminalPerGlyphUniforms *glyph_uniforms [[buffer(1)]],
        constant TerminalLayerUniforms *layer [[buffer(2)]])
{
    const device TerminalPerGlyphUniforms *glyph = &glyph_uniforms[instance_id];
    TerminalGlyphTransform transform = terminal_glyph_transform(glyph, layer);
    float2 local = (vertices[vertex_id] - 0.5) * transform.scale + 0.5;
    float2 pixel_pos = local * glyph->size + glyph->origin + transform.offset;
    pixel_pos.y = floor(pixel_pos.y);

    float fade_width = fabs(glyph->fade_end - glyph->fade_start);
    float fade_dist = pixel_pos.x - min(glyph->fade_start, glyph->fade_end);
    float fade_alpha = glyph->fade_end < glyph->fade_start
        ? fade_dist / max(fade_width, 0.001)
        : 1.0 - fade_dist / max(fade_width, 0.001);
    float2 device_pos = pixel_pos / layer->viewport_size * float2(2.0, -2.0) + float2(-1.0, 1.0);

    GlyphFragmentData out;
    out.position = float4(device_pos, 0.0, 1.0);
    out.rect_corner = glyph->size / 2.0;
    out.rect_center = glyph->origin + out.rect_corner;
    out.texture_coordinate = float2(glyph->uv_left, glyph->uv_top)
        + vertices[vertex_id] * float2(glyph->uv_width, glyph->uv_height);
    out.fade_alpha = fade_alpha * transform.alpha;
    out.color = glyph->color;
    out.is_emoji = glyph->is_emoji;
    out.terminal_glow = transform.glow;
    out.glyph_local = vertices[vertex_id];
    out.terminal_progress = transform.progress;
    out.terminal_seed = glyph->seed;
    out.terminal_flags = glyph->flags;
    return out;
}

fragment float4 terminal_glyph_fragment_shader(
    GlyphFragmentData in [[stage_in]],
    texture2d<half> color_texture [[texture(0)]])
{
    constexpr sampler texture_sampler(mag_filter::linear, min_filter::linear);
    float4 sample = float4(color_texture.sample(texture_sampler, in.texture_coordinate));
    float4 color = mix(in.color, sample, float(in.is_emoji));
    color.a *= max(sample.r, float(in.is_emoji)) * saturate(in.fade_alpha);
    if ((in.terminal_flags & 2u) != 0u) {
        float2 shard = floor(in.glyph_local * float2(8.0, 10.0));
        float noise = fract(sin(dot(shard + in.terminal_seed * 17.0,
            float2(12.9898, 78.233))) * 43758.5453);
        float dissolve = smoothstep(0.12, 0.9, in.terminal_progress);
        float survival = smoothstep(dissolve - 0.1, dissolve + 0.08, noise);
        float final_fade = 1.0 - smoothstep(0.74, 1.0, in.terminal_progress);
        color.a *= survival * final_fade;
        color.rgb = mix(color.rgb, float3(1.0), (1.0 - survival) * 0.08);
    }
    color.rgb = mix(color.rgb, float3(1.0), in.terminal_glow);
    color.a = min(1.0, color.a + in.terminal_glow * 0.12);
    return color;
}

vertex TerminalOverlayFragmentData terminal_substrate_vertex_shader(
        uint vertex_id [[vertex_id]],
        constant float2 *vertices [[buffer(0)]],
        constant TerminalLayerUniforms *layer [[buffer(1)]])
{
    float2 pixel_pos = vertices[vertex_id] * layer->terminal_viewport_size + layer->viewport_origin;
    float2 device_pos = pixel_pos / layer->viewport_size * float2(2.0, -2.0) + float2(-1.0, 1.0);
    TerminalOverlayFragmentData out;
    out.position = float4(device_pos, 0.0, 1.0);
    out.local_position = vertices[vertex_id];
    out.color = layer->material_tint;
    out.kind = 0u;
    out.effect_kind = 0u;
    out.progress = 0.0;
    out.seed = 0.0;
    return out;
}

fragment float4 terminal_substrate_fragment_shader(
        TerminalOverlayFragmentData in [[stage_in]],
        constant TerminalLayerUniforms *layer [[buffer(0)]])
{
    if ((layer->material_flags & 1u) == 0u) return float4(0.0);
    float grain = fract(sin(dot(in.local_position * layer->terminal_viewport_size,
        float2(12.9898, 78.233))) * 43758.5453) - 0.5;
    float edge = smoothstep(0.0, 0.09, in.local_position.x)
        * smoothstep(0.0, 0.09, 1.0 - in.local_position.x);
    float style_strength = 0.028 + float(layer->material_style) * 0.008;
    float foreground_luma = dot(layer->terminal_foreground.rgb, float3(0.2126, 0.7152, 0.0722));
    float tint_luma = dot(layer->material_tint.rgb, float3(0.2126, 0.7152, 0.0722));
    float readability = smoothstep(0.12, 0.52, abs(foreground_luma - tint_luma));
    float4 color = layer->material_tint;
    color.rgb += grain * 0.012;
    color.a = style_strength * (0.86 + edge * 0.14) * mix(0.22, 1.0, readability);
    return color;
}

vertex TerminalOverlayFragmentData terminal_block_vertex_shader(
        uint vertex_id [[vertex_id]],
        uint instance_id [[instance_id]],
        constant float2 *vertices [[buffer(0)]],
        const device TerminalBlockUniforms *blocks [[buffer(1)]],
        constant TerminalLayerUniforms *layer [[buffer(2)]])
{
    const device TerminalBlockUniforms *block = &blocks[instance_id];
    float progress = saturate((layer->elapsed_seconds - block->start_seconds)
        / max(layer->duration_seconds, 0.001));
    float2 local = vertices[vertex_id];
    uint variant = block->effect_kind % 100;
    if (block->effect_kind != 0) {
        float eased = terminal_ease_out(progress);
        if (variant == 1) local.y *= max(0.02, eased);
        if (variant == 2) local.y = saturate((local.y - (1.0 - eased) * block->seed) / max(eased, 0.02));
        if (variant == 3) local.x = saturate(local.x / max(eased, 0.02));
        if (variant == 4) local.y = 0.5 + (local.y - 0.5) * eased;
        if (variant == 5) local.y += sin(local.x * M_PI_F) * (1.0 - eased) * 0.08;
        if (variant == 6) local.y += (1.0 - eased) * 0.12 * block->phase;
    }
    float2 pixel_pos = local * block->size + block->origin;
    float2 device_pos = pixel_pos / layer->viewport_size * float2(2.0, -2.0) + float2(-1.0, 1.0);
    TerminalOverlayFragmentData out;
    out.position = float4(device_pos, 0.0, 1.0);
    out.local_position = local;
    out.color = layer->material_accent;
    out.kind = 0u;
    out.effect_kind = 0u;
    out.progress = 0.0;
    out.seed = 0.0;
    return out;
}

fragment float4 terminal_block_fragment_shader(
        TerminalOverlayFragmentData in [[stage_in]],
        constant TerminalLayerUniforms *layer [[buffer(0)]])
{
    bool gutter_enabled = (layer->material_flags & 2u) != 0u;
    bool depth_enabled = (layer->material_flags & 4u) != 0u;
    float gutter = gutter_enabled ? 1.0 - smoothstep(0.0, 0.012, in.local_position.x) : 0.0;
    float top = 1.0 - smoothstep(0.0, 0.045, in.local_position.y);
    float bottom = smoothstep(0.955, 1.0, in.local_position.y);
    float depth = depth_enabled ? max(top, bottom * 0.72) : 0.0;
    float foreground_luma = dot(layer->terminal_foreground.rgb, float3(0.2126, 0.7152, 0.0722));
    float tint_luma = dot(layer->material_tint.rgb, float3(0.2126, 0.7152, 0.0722));
    float contrast_guard = clamp(abs(foreground_luma - tint_luma) * 1.7, 0.2, 0.9);
    float alpha = (gutter * 0.11 + depth * 0.055) * contrast_guard;
    return float4(layer->material_accent.rgb, alpha);
}

vertex TerminalOverlayFragmentData terminal_overlay_vertex_shader(
        uint vertex_id [[vertex_id]],
        uint instance_id [[instance_id]],
        constant float2 *vertices [[buffer(0)]],
        const device TerminalOverlayUniforms *items [[buffer(1)]],
        constant TerminalLayerUniforms *layer [[buffer(2)]])
{
    const device TerminalOverlayUniforms *item = &items[instance_id];
    uint category = item->effect_kind / 100;
    uint variant = item->effect_kind % 100;
    float life_seed = terminal_hash(float2(item->seed * 37.0 + float(variant), item->origin.x * 0.11 + item->origin.y * 0.07));
    float duration_scale = item->kind == 1u ? clamp(item->seed, 0.05, 1.0) : mix(1.35, 2.0, life_seed);
    float duration = max(layer->duration_seconds * duration_scale, 0.001);
    float progress = saturate((layer->elapsed_seconds - item->start_seconds) / duration);
    float eased = terminal_ease_out(progress);
    float motion_progress = eased;
    float2 size = max(item->size, float2(0.75));
    float2 base_size = size;
    float2 velocity = item->velocity;
    float2 origin_adjustment = float2(0.0);
    if (item->kind == 1u) {
        size = item->size;
        if (variant == 0) motion_progress = 1.0;
        if (variant == 2) motion_progress = terminal_spring(progress);
        if (variant == 3) {
            float stretch = abs(velocity.x) * (1.0 - eased) * 0.32;
            size.x += stretch;
            if (velocity.x < 0.0) origin_adjustment.x -= stretch;
        }
        if (variant == 4) {
            float squash = sin(progress * M_PI_F) * 0.22;
            size.x *= 1.0 + squash;
            size.y *= 1.0 - squash * 0.55;
        }
        if (variant == 6) velocity.y -= sin(progress * M_PI_F) * layer->cell_size.y * 0.42;
        if (variant == 7) motion_progress = smoothstep(0.08, 0.92, progress);
        if (variant == 8) size.x = max(size.x, 2.0) * (0.78 + eased * 0.22);
    } else if (category == 5) {
        if (variant == 1) size *= 1.0 + progress * 2.2;
        if (variant == 2) velocity.y = abs(velocity.y) * 0.35;
        if (variant == 3) velocity.x *= sin(progress * M_PI_F * 2.0);
        if (variant == 4) size.y *= 1.0 + progress * 3.0;
        if (variant == 5) velocity.x *= 2.2;
        if (variant == 6) motion_progress *= 0.45;
        if (variant == 7) size *= 0.6 + sin(progress * M_PI_F) * 1.8;
        if (variant == 8) velocity = -velocity;
        if (variant == 9) velocity.x = abs(velocity.x) + 20.0;
        if (variant == 10) velocity.y = abs(velocity.y) + 18.0;
        if (variant == 12) {
            bool residual = item->seed >= 0.82;
            motion_progress = 1.0 - exp(-3.8 * progress);
            velocity.x = abs(velocity.x) * (residual ? 0.7 : 1.0);
            velocity.y = -abs(velocity.y) * (residual ? 0.7 : 1.0);
            size *= residual ? 0.96 - progress * 0.18 : 1.0 - progress * 0.38;
            origin_adjustment.x += sin(progress * M_PI_F * 2.0 + item->seed * 8.0)
                * (1.0 - progress) * (residual ? 0.35 : 0.7);
        }
        if (variant == 13) size *= 1.0 - progress * 0.78;
        if (variant == 14) velocity.y = abs(velocity.y) + progress * 42.0;
        if (variant == 15) {
            motion_progress = progress;
            velocity.x = (item->seed - 0.5) * 5.0;
            velocity.y = -10.0 - item->seed * 16.0;
            origin_adjustment.x += sin(progress * 19.0 + item->seed * 23.0) * (1.0 - progress) * 2.2;
            origin_adjustment.y -= progress * progress * (8.0 + item->seed * 10.0);
            size.x *= 0.55 + item->seed * 0.35;
            size.y *= 1.05 + progress * 1.75;
            origin_adjustment.y -= size.y - base_size.y;
        }
        if (variant == 16) {
            motion_progress = 1.0 - exp(-2.8 * progress);
            velocity.x = 16.0 + item->seed * 18.0;
            velocity.y = -2.0 - item->seed * 5.0;
            origin_adjustment.x += progress * progress * (7.0 + item->seed * 7.0);
            origin_adjustment.y += sin(progress * M_PI_F * 5.0 + item->seed * 9.0) * 2.4;
            size *= 0.46 + item->seed * 0.2;
        }
        if (variant == 17) {
            motion_progress = progress;
            velocity.x = (item->seed - 0.5) * 2.4;
            velocity.y = -3.0 - item->seed * 4.0;
            origin_adjustment.y += progress * progress * (24.0 + item->seed * 18.0);
            size.x *= 0.62 - progress * 0.12;
            size.y *= 1.0 + progress * 1.25;
        }
        if (variant == 18) {
            velocity = float2(0.0);
            motion_progress = smoothstep(0.0, 1.0, progress);
            size = base_size * (1.0 + motion_progress * (4.0 + item->seed * 1.4));
            origin_adjustment.y -= (size.y - base_size.y) * 0.5;
        }
        if (variant == 19) {
            motion_progress = 1.0 - exp(-2.4 * progress);
            velocity.x = (item->seed - 0.5) * 5.0;
            velocity.y = -5.0 - item->seed * 8.0;
            origin_adjustment.x += sin(progress * M_PI_F * 3.5 + item->seed * 11.0) * 3.2;
            origin_adjustment.y -= progress * progress * (2.0 + item->seed * 4.0);
            size *= 0.48 + item->seed * 0.34 + progress * 0.12;
        }
        if (variant == 20) {
            motion_progress = progress;
            velocity.x = 3.0 + item->seed * 13.0;
            velocity.y = -8.0 - item->seed * 14.0;
            origin_adjustment.y += progress * progress * (14.0 + item->seed * 18.0);
            size.x *= 0.45;
            size.y *= 1.2 + item->seed;
        }
        if (variant == 21) {
            float angle = item->seed * M_PI_F * 2.0;
            motion_progress = 1.0 - exp(-3.0 * progress);
            velocity = float2(cos(angle), sin(angle)) * (2.0 + item->seed * 3.0);
            size = base_size * (1.0 + progress * (1.7 + item->seed));
            origin_adjustment.y -= (size.y - base_size.y) * 0.5;
        }
        if (variant == 22) {
            motion_progress = 1.0 - exp(-2.2 * progress);
            velocity.x = (item->seed - 0.5) * 4.0;
            velocity.y = -4.0 - item->seed * 8.0;
            origin_adjustment.x += sin(progress * M_PI_F * 2.6 + item->seed * 8.0) * 2.6;
            origin_adjustment.y -= progress * progress * (3.0 + item->seed * 4.0);
            size = base_size * (0.82 + progress * (1.45 + item->seed));
            origin_adjustment.y -= (size.y - base_size.y) * 0.5;
        }
        if (variant == 23) {
            velocity.x = round((1.0 + item->seed * 5.0) / 2.0) * 2.0;
            velocity.y = -round((3.0 + item->seed * 7.0) / 2.0) * 2.0;
            size = max(round(size / 2.0) * 2.0, float2(1.0));
            motion_progress = floor(eased * 8.0) / 8.0;
        }
    }
    if (category == 5 && variant != 18 && variant != 23) {
        float envelope = sin(progress * M_PI_F);
        float turbulence = sin(progress * (7.0 + life_seed * 11.0) + item->seed * 31.0);
        origin_adjustment += float2(turbulence * (0.7 + life_seed * 1.9),
            (life_seed - 0.5) * 2.2) * envelope;
    }
    float motion_scale = item->kind == 0u ? layer->particle_spread * 0.8 : 1.0;
    if (item->kind == 0u && category == 5 && variant == 18) size = base_size + (size - base_size) * motion_scale;
    float2 origin = item->origin + (velocity * motion_progress + origin_adjustment) * motion_scale;
    float2 local = vertices[vertex_id];
    if (item->kind == 0u) {
        float spin = category == 5 && variant == 20 ? 1.35 : 0.42;
        if (category == 5 && (variant == 15 || variant == 17 || variant == 18 || variant == 23))
            spin = 0.0;
        float angle = (item->seed - 0.5) * spin * smoothstep(0.15, 1.0, progress);
        float sine = sin(angle);
        float cosine = cos(angle);
        float2 centered = local - 0.5;
        local = float2(
            centered.x * cosine - centered.y * sine,
            centered.x * sine + centered.y * cosine
        ) + 0.5;
    }
    float2 pixel_pos = local * size + origin;
    float2 device_pos = pixel_pos / layer->viewport_size * float2(2.0, -2.0) + float2(-1.0, 1.0);
    TerminalOverlayFragmentData out;
    out.position = float4(device_pos, 0.0, 1.0);
    out.local_position = vertices[vertex_id];
    out.color = item->color;
    out.kind = item->kind;
    out.effect_kind = item->effect_kind;
    out.progress = progress;
    out.seed = item->seed;
    if (item->kind == 0u) {
        bool erase_residual = category == 5 && variant == 12 && item->seed >= 0.82;
        float fade_seed = terminal_hash(float2(item->seed * 19.0 + float(variant), item->origin.y * 0.09));
        float reveal = smoothstep(0.0, mix(0.025, 0.085, fade_seed), progress);
        float fade = 1.0 - smoothstep(erase_residual ? mix(0.42, 0.62, fade_seed) : mix(0.18, 0.4, fade_seed), erase_residual ? 1.0 : mix(0.78, 1.0, life_seed), progress);
        if (category == 5 && variant == 12 && !erase_residual) fade = 1.0 - smoothstep(mix(0.16, 0.34, fade_seed), mix(0.84, 1.0, life_seed), progress); if (category == 5 && variant == 15) fade = 1.0 - smoothstep(0.42, 1.0, progress);
        if (category == 5 && variant == 16) fade = 1.0 - smoothstep(0.3, 0.9, progress);
        if (category == 5 && variant == 17) fade = 1.0 - smoothstep(0.58, 1.0, progress);
        if (category == 5 && variant == 18) fade = 1.0 - smoothstep(0.3, 0.96, progress);
        if (category == 5 && variant == 19) fade = 1.0 - smoothstep(0.46, 1.0, progress);
        if (category == 5 && variant == 20) fade = 1.0 - smoothstep(0.34, 0.92, progress);
        if (category == 5 && variant == 21) fade = 1.0 - smoothstep(0.36, 1.0, progress);
        if (category == 5 && variant == 22) fade = 1.0 - smoothstep(0.5, 1.0, progress);
        if (category == 5 && variant == 23) fade = 1.0 - smoothstep(0.24, 0.88, progress);
        float opacity_motion = 0.92 + sin(progress * (5.0 + life_seed * 4.0) + item->seed * 17.0) * 0.08;
        out.color.a *= reveal * fade * mix(0.58, 1.0, fade_seed) * opacity_motion;
    }
    return out;
}

fragment float4 terminal_overlay_fragment_shader(TerminalOverlayFragmentData in [[stage_in]]) {
    if (in.kind == 1u) return in.color;
    uint variant = in.effect_kind % 100;
    float2 centered = in.local_position - 0.5;
    float square_distance = max(abs(centered.x), abs(centered.y));
    float radial_distance = length(centered);
    float mask = 1.0 - smoothstep(0.26, 0.5, square_distance);
    float3 color = in.color.rgb;

    if (variant == 12) {
        bool residual = in.seed >= 0.82;
        float edge = residual ? 0.43 : 0.44;
        mask = 1.0 - smoothstep(edge, 0.49, square_distance);
        float shard_noise = terminal_hash(floor(in.local_position * float2(4.0, 5.0)) + in.seed);
        if (residual) mask *= 0.72 + shard_noise * 0.28;
        color = mix(color, float3(1.0), residual ? 0.05 : 0.08 + shard_noise * 0.1);
    } else if (variant == 15) {
        float flame = 1.0 - smoothstep(0.22, 0.52,
            length(float2(centered.x * 1.45, centered.y * 0.78)));
        float flicker = 0.72 + 0.28 * sin(in.progress * 24.0 + in.seed * 19.0);
        mask = flame * flicker;
        color = mix(float3(1.0, 0.16, 0.025), float3(1.0, 0.82, 0.18),
            saturate(1.0 - in.local_position.y + in.seed * 0.28));
    } else if (variant == 16) {
        mask = 1.0 - smoothstep(0.18, 0.46, radial_distance);
        mask *= 0.58 + terminal_hash(in.local_position * 13.0 + in.seed) * 0.42;
        color = mix(float3(0.5, 0.31, 0.12), float3(0.94, 0.76, 0.38), in.seed);
    } else if (variant == 17) {
        mask = 1.0 - smoothstep(0.24, 0.5,
            length(float2(centered.x * 1.35, centered.y * 0.72)));
        float highlight = 1.0 - smoothstep(0.05, 0.2, length(in.local_position - float2(0.36, 0.3)));
        color = mix(float3(0.08, 0.42, 0.86), float3(0.55, 0.92, 1.0), 0.35 + highlight * 0.65);
    } else if (variant == 18) {
        float ring = 1.0 - smoothstep(0.035, 0.085, abs(radial_distance - 0.37));
        mask = ring * (1.0 - smoothstep(0.55, 1.0, in.progress));
        color = mix(float3(0.58, 0.28, 1.0), float3(0.18, 0.86, 1.0), in.seed);
    } else if (variant == 19) {
        mask = 1.0 - smoothstep(0.16, 0.5, radial_distance);
        mask *= 0.55 + terminal_hash(in.local_position * 9.0 + in.seed * 3.0) * 0.45;
        float luminance = dot(color, float3(0.2126, 0.7152, 0.0722));
        color = mix(float3(luminance), float3(0.7, 0.72, 0.75), 0.42);
    } else if (variant == 20) {
        float diamond = abs(centered.x) + abs(centered.y);
        mask = 1.0 - smoothstep(0.28, 0.5, diamond);
        float glint = pow(max(0.0, 1.0 - abs(centered.x + centered.y) * 4.0), 5.0);
        color = mix(float3(0.2, 0.62, 0.94), float3(0.92, 0.98, 1.0), glint);
    } else if (variant == 21) {
        float noise = terminal_hash(floor(in.local_position * 8.0) + in.seed * 11.0);
        mask = 1.0 - smoothstep(0.28 + noise * 0.08, 0.5, radial_distance);
        color = mix(float3(0.05, 0.04, 0.12), float3(0.32, 0.12, 0.48), in.seed);
    } else if (variant == 22) {
        mask = 1.0 - smoothstep(0.08, 0.52, radial_distance);
        mask *= 0.72 - in.progress * 0.22;
        color = mix(color, float3(0.55, 0.64, 0.72), 0.58);
    } else if (variant == 23) {
        mask = 1.0 - step(0.46, square_distance);
        color = mix(color, float3(1.0), 0.08 + in.seed * 0.22);
    }
    return float4(color, in.color.a * saturate(mask));
}
#endif
