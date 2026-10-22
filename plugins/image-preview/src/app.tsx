import { createSignal, onMount, Show } from "solid-js";
import { getOps, mount, registerTexture } from "@pocketjs/framework";
import { Image, Screen, Text, View } from "@pocketjs/framework/components";
import { onFrame } from "@pocketjs/framework/lifecycle";
import { ContainerController } from "../../sdk/container";
import {
  CONTENT_PADDING,
  DARK_PALETTE,
  FAILURE_CONTENT_SIZE,
  METADATA_HEIGHT,
  ROOT_SURFACE_ID,
  type PreviewSize,
  type SurfacePalette,
  containerPage,
  contentSurfaceSize,
  fittedPreviewSize,
  shortened,
  surfacePalette,
} from "./presentation";

const PLUGIN_ID = "dev.warpeer.image-preview";
const CONTAINER_ID = "10000000-0000-4000-8000-000000000002";
const STATIC_GLYPHS = "Image Preview Loading Preview unavailable PNG JPEG Local HTTPS 0123456789 x . / -";
const MINIMUM_HOVER_DELAY_MS = 1500;

type HostMessage = { type: string; payload?: any };
type HoverContext = {
  context_id: string;
  revision: number;
  pane_id: string;
  resource_text: string;
  resolved_resource?: string;
  explicit_preview?: boolean;
  anchor: [number, number, number, number];
  viewport: [number, number, number, number];
};

type PluginOps = ReturnType<typeof getOps> & {
  svcPoll?: () => string | undefined;
  svcSend?: (line: string) => void;
};

function hostOps(): PluginOps {
  return getOps() as PluginOps;
}

(globalThis as typeof globalThis & {
  __warpeerRegisterImage?: (url: string, handle: number) => void;
}).__warpeerRegisterImage = (url, handle) => registerTexture(url, handle);

let requestSequence = 1;
let currentContext: HoverContext | null = null;
let pendingHover: { context: HoverContext; due: number } | null = null;
let pendingSurface: { context: HoverContext; due: number; size: PreviewSize } | null = null;
let frameNumber = 0;
let activeRequest = "";
let rootSurfaceOpen = false;

function uuid(sequence: number): string {
  return `20000000-0000-4000-8000-${sequence.toString(16).padStart(12, "0")}`;
}

function send(message: unknown): void {
  hostOps().svcSend?.(JSON.stringify(message));
}

const container = new ContainerController(CONTAINER_ID, send, ["chrome"]);

function openSurface(context: HoverContext, size: PreviewSize): void {
  send({
    type: "open-surface",
    payload: {
      id: ROOT_SURFACE_ID,
      slot: "terminal-hover",
      target: {
        kind: "terminal-context",
        context_id: context.context_id,
        pane_id: context.pane_id,
      },
      revision: context.revision,
      bounds: { x: 0, y: 0, ...contentSurfaceSize(size), scale: 1 },
      accessibility_label: "Image preview",
      accepts_pointer: false,
      accepts_keyboard: false,
    },
  });
  rootSurfaceOpen = true;
  if (context.explicit_preview && container.supports("chrome")) {
    container.resetRuntimeState();
    container.open(
      context.context_id,
      context.revision,
      "terminal-hover",
      containerPage(size),
      { style: "none", duration_ms: 0, easing: "linear" },
    );
  }
}

function closeSurface(context: HoverContext): void {
  if (!rootSurfaceOpen) return;
  send({
    type: "close-surface",
    payload: { surface_id: ROOT_SURFACE_ID, revision: context.revision },
  });
  rootSurfaceOpen = false;
}

function requestPreview(context: HoverContext): void {
  const target = context.resolved_resource ?? context.resource_text;
  const remote = target.toLowerCase().startsWith("https://");
  activeRequest = uuid(requestSequence++);
  currentContext = context;
  send({
    type: "capability-request",
    payload: {
      request_id: activeRequest,
      plugin_id: PLUGIN_ID,
      context_id: context.context_id,
      context_revision: context.revision,
      method: remote ? "network.fetch.preview-image" : "resource.read.preview-image",
      scope: remote ? "https-image" : "exact-image-file",
      payload: { target },
    },
  });
}

function supported(context: HoverContext): boolean {
  const target = (context.resolved_resource ?? context.resource_text).toLowerCase();
  if (target.startsWith("https://")) return true;
  const path = target.split(/[?#]/)[0];
  return path.endsWith(".png") || path.endsWith(".jpg") || path.endsWith(".jpeg");
}

function App() {
  const [phase, setPhase] = createSignal<"loading" | "ready" | "failed">("loading");
  const [imageUrl, setImageUrl] = createSignal("");
  const [contentSize, setContentSize] = createSignal<PreviewSize>(FAILURE_CONTENT_SIZE);
  const [metadata, setMetadata] = createSignal("Loading preview");
  const [hoverDelay, setHoverDelay] = createSignal(MINIMUM_HOVER_DELAY_MS);
  const [triggerMode, setTriggerMode] = createSignal("click menu");
  const [palette, setPalette] = createSignal(DARK_PALETTE);

  function handle(message: HostMessage): void {
    const containerMessage = container.handle(message);
    if (message.type === "container-event" && message.payload?.container_id === CONTAINER_ID) {
      const event = message.payload;
      if (event.type === "dismissed" || event.type === "cancelled") {
        if (currentContext) closeSurface(currentContext);
        container.resetRuntimeState();
      }
    }
    if (containerMessage) return;
    if (message.type === "configuration") {
      const value = Number(message.payload?.values?.hover_delay_ms);
      if (Number.isFinite(value)) setHoverDelay(Math.max(MINIMUM_HOVER_DELAY_MS, value));
      const mode = String(message.payload?.values?.trigger_mode ?? "Click menu").toLowerCase();
      setTriggerMode(mode);
      return;
    }
    if (message.type === "terminal-hover") {
      const context = message.payload as HoverContext;
      if (!supported(context)) return;
      if (!context.explicit_preview && triggerMode() === "click menu") return;
      setPhase("loading");
      setMetadata("Loading preview");
      pendingHover = {
        context,
        due: context.explicit_preview
          ? frameNumber
          : frameNumber + Math.ceil(hoverDelay() / (1000 / 60)),
      };
      return;
    }
    if (message.type === "surface-environment") {
      setPalette(surfacePalette(message.payload?.palette));
      return;
    }
    if (message.type === "release-surface") {
      if (message.payload?.surface_id === ROOT_SURFACE_ID) rootSurfaceOpen = false;
      return;
    }
    if (message.type === "cancel-context") {
      if (currentContext?.context_id === message.payload?.context_id) {
        closeSurface(currentContext);
        currentContext = null;
        activeRequest = "";
        container.resetRuntimeState();
      }
      pendingHover = null;
      pendingSurface = null;
      return;
    }
    if (message.type !== "capability-response" || message.payload?.request_id !== activeRequest) {
      return;
    }
    const success = message.payload?.result?.Ok;
    const value = success?.kind === "json" ? success.value : null;
    if (typeof value?.url === "string") {
      const decodedSize = { width: Number(value.width), height: Number(value.height) };
      const fittedSize = currentContext ? fittedPreviewSize(currentContext, decodedSize) : decodedSize;
      const originalWidth = Number(value.originalWidth ?? value.width);
      const originalHeight = Number(value.originalHeight ?? value.height);
      const mediaType = value.mediaType === "image/png" ? "PNG" : "JPEG";
      const reference = currentContext?.resource_text ?? "Image";
      const source = (currentContext?.resolved_resource ?? reference).toLowerCase().startsWith("https://")
        ? "HTTPS"
        : "Local";
      setImageUrl(value.url);
      setContentSize(fittedSize);
      setMetadata(`${originalWidth} x ${originalHeight}  |  ${mediaType}  |  ${source}  |  ${shortened(reference)}`);
      setPhase("ready");
      if (currentContext) {
        pendingSurface = { context: currentContext, due: frameNumber + 1, size: fittedSize };
      }
    } else {
      setPhase("failed");
      setContentSize(FAILURE_CONTENT_SIZE);
      setMetadata("Preview unavailable");
      if (currentContext) {
        pendingSurface = {
          context: currentContext,
          due: frameNumber + 1,
          size: FAILURE_CONTENT_SIZE,
        };
      }
    }
  }

  onMount(() => {
    onFrame(() => {
      frameNumber += 1;
      const batch = hostOps().svcPoll?.();
      if (batch) {
        for (const line of batch.split("\n")) {
          if (!line) continue;
          try { handle(JSON.parse(line) as HostMessage); } catch {}
        }
      }
      if (pendingHover && frameNumber >= pendingHover.due) {
        const context = pendingHover.context;
        pendingHover = null;
        requestPreview(context);
      }
      if (pendingSurface && frameNumber >= pendingSurface.due) {
        const { context, size } = pendingSurface;
        pendingSurface = null;
        if (currentContext?.context_id === context.context_id) openSurface(context, size);
      }
    });
  });

  return (
    <Screen
      class="relative w-full h-full flex-col overflow-hidden"
      style={{ bgColor: palette().background }}
    >
      <View
        class="relative items-center justify-center"
        style={{
          width: contentSize().width + CONTENT_PADDING * 2,
          height: contentSize().height + CONTENT_PADDING * 2,
          bgColor: palette().background,
        }}
      >
        <Show when={phase() === "ready"}>
          <Image
            src={imageUrl()}
            style={{ width: contentSize().width, height: contentSize().height }}
          />
        </Show>
        <Show when={phase() !== "ready"}>
          <View class="absolute px-4 py-3" style={{ bgColor: palette().surface }}>
            <Text class="text-xs" style={{ textColor: palette().mutedText }}>{metadata()}</Text>
          </View>
        </Show>
      </View>
      <View class="flex-col" style={{ height: METADATA_HEIGHT, bgColor: palette().surface }}>
        <View
          style={{
            width: contentSize().width + CONTENT_PADDING * 2,
            height: 1,
            bgColor: palette().border,
          }}
        />
        <View class="flex-row items-center px-3" style={{ height: METADATA_HEIGHT - 1 }}>
          <Text class="text-xs" style={{ textColor: palette().mutedText }}>{metadata()}</Text>
        </View>
      </View>
      <Text class="absolute w-[1] h-[1]" style={{ opacity: 0 }}>{STATIC_GLYPHS}</Text>
    </Screen>
  );
}

mount(() => <App />);
