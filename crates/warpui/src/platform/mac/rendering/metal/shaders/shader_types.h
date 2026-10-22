#ifndef shader_types_h
#define shader_types_h

#include <simd/simd.h>

typedef struct {
  vector_float2 viewport_size;
} Uniforms;

typedef struct {
  vector_float2 origin;
  vector_float2 size;
  float corner_radius_top_left;
  float corner_radius_top_right;
  float corner_radius_bottom_left;
  float corner_radius_bottom_right;
  float border_top;
  float border_right;
  float border_bottom;
  float border_left;
  vector_float2 background_start;
  vector_float2 background_end;
  vector_float4 background_start_color;
  vector_float4 background_end_color;
  vector_float2 border_start;
  vector_float2 border_end;
  vector_float4 border_start_color;
  vector_float4 border_end_color;
  vector_float4 icon_color;
  int is_icon;
  vector_float2 drop_shadow_offsets;
  vector_float4 drop_shadow_color;
  float drop_shadow_sigma;
  float drop_shadow_padding_factor;
  float dash_length;
  vector_float2 gap_lengths;
} PerRectUniforms;

typedef struct {
  vector_float2 origin;
  vector_float2 size;
  float uv_left;
  float uv_top;
  float uv_width;
  float uv_height;
  float fade_start;
  float fade_end;
  vector_float4 color;
  int is_emoji;
} PerGlyphUniforms;

#ifdef TERMINAL_VISUALS
typedef struct {
  vector_float2 viewport_size;
  vector_float2 viewport_origin;
  vector_float2 terminal_viewport_size;
  vector_float2 cell_size;
  float elapsed_seconds;
  float duration_seconds;
  unsigned int new_line_effect;
  unsigned int commit_effect;
  unsigned int keypress_effect;
  unsigned int cursor_effect;
  unsigned int erase_effect;
  unsigned int block_effect;
  unsigned int material_style;
  unsigned int material_flags;
  float particle_spread;
  vector_float4 material_tint;
  vector_float4 material_accent;
  vector_float4 terminal_foreground;
} TerminalLayerUniforms;

typedef struct {
  vector_float2 origin;
  vector_float2 size;
  float uv_left;
  float uv_top;
  float uv_width;
  float uv_height;
  float fade_start;
  float fade_end;
  vector_float4 color;
  int is_emoji;
  vector_float2 current_cell_origin;
  vector_float2 previous_cell_origin;
  float start_seconds;
  unsigned int effect_kind;
  float seed;
  unsigned int flags;
} TerminalPerGlyphUniforms;

typedef struct {
  vector_float2 origin;
  vector_float2 velocity;
  vector_float2 size;
  float start_seconds;
  float seed;
  unsigned int effect_kind;
  unsigned int kind;
  vector_float4 color;
} TerminalOverlayUniforms;

typedef struct {
  vector_float2 origin;
  vector_float2 size;
  float start_seconds;
  unsigned int effect_kind;
  float phase;
  float seed;
} TerminalBlockUniforms;
#endif

#endif // shader_types_h
