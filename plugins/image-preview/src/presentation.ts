import type { ContainerPage } from "../../sdk/container";

export const ROOT_SURFACE_ID = "10000000-0000-4000-8000-000000000001";
export const ROOT_PAGE_ID = "10000000-0000-4000-8000-000000000003";
export const CONTENT_PADDING = 12;
export const METADATA_HEIGHT = 28;
const VIEWPORT_MARGIN = 8;

export type PreviewSize = { width: number; height: number };
export type SurfacePalette = {
  background: string;
  surface: string;
  text: string;
  mutedText: string;
  border: string;
  accent: string;
};

export const FAILURE_CONTENT_SIZE: PreviewSize = { width: 272, height: 32 };
export const DARK_PALETTE: SurfacePalette = {
  background: "#0f1115ff",
  surface: "#1c1f25ff",
  text: "#f2f4f8ff",
  mutedText: "#a1a6b0ff",
  border: "#454a54ff",
  accent: "#5b9cffff",
};

export function fittedPreviewSize(
  viewport: { viewport: [number, number, number, number] },
  image: PreviewSize,
): PreviewSize {
  const [, , viewportWidth, viewportHeight] = viewport.viewport;
  const maxWidth = Math.max(1, viewportWidth - VIEWPORT_MARGIN * 4 - CONTENT_PADDING * 2);
  const maxHeight = Math.max(
    1,
    viewportHeight - VIEWPORT_MARGIN * 4 - METADATA_HEIGHT - CONTENT_PADDING * 2,
  );
  const maxScale = Math.min(maxWidth / image.width, maxHeight / image.height);
  const comfortableScale = Math.max(1, 240 / image.width);
  const scale = Math.min(maxScale, comfortableScale);
  return {
    width: Math.max(1, Math.round(image.width * scale)),
    height: Math.max(1, Math.round(image.height * scale)),
  };
}

export function contentSurfaceSize(content: PreviewSize): PreviewSize {
  return {
    width: content.width + CONTENT_PADDING * 2,
    height: content.height + CONTENT_PADDING * 2 + METADATA_HEIGHT,
  };
}

export function containerPage(size: PreviewSize): ContainerPage {
  return {
    id: ROOT_PAGE_ID,
    revision: 1,
    surface_id: ROOT_SURFACE_ID,
    chrome: {
      visibility: "visible",
      title: "Image Preview",
      leading: [],
      trailing: [],
      drag_regions: [],
    },
    preferred_content_size: contentSurfaceSize(size),
  };
}

function paletteColor(value: unknown, fallback: string): string {
  if (!Array.isArray(value) || value.length !== 4) return fallback;
  const channels = value.map((channel) => Math.max(0, Math.min(255, Number(channel) || 0)));
  return `#${channels.map((channel) => Math.round(channel).toString(16).padStart(2, "0")).join("")}`;
}

export function surfacePalette(value: any): SurfacePalette {
  const background = paletteColor(value?.background, DARK_PALETTE.background);
  const surface = paletteColor(value?.surface, DARK_PALETTE.surface);
  return {
    background: `${background.slice(0, 7)}ff`,
    surface: `${surface.slice(0, 7)}ff`,
    text: paletteColor(value?.text, DARK_PALETTE.text),
    mutedText: paletteColor(value?.muted_text, DARK_PALETTE.mutedText),
    border: paletteColor(value?.border, DARK_PALETTE.border),
    accent: paletteColor(value?.accent, DARK_PALETTE.accent),
  };
}

export function shortened(value: string, maximum = 48): string {
  if (value.length <= maximum) return value;
  const head = Math.max(8, Math.floor((maximum - 3) * 0.45));
  return `${value.slice(0, head)}...${value.slice(value.length - (maximum - head - 3))}`;
}
