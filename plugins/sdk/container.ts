export type ContainerExtension =
  | "chrome"
  | "resize-transition"
  | "drag-regions";

export type ChromeAction = {
  id: string;
  role: "action" | "toggle" | "menu";
  icon: string;
  label: string;
  tooltip?: string;
  enabled?: boolean;
  selected?: boolean;
  menu?: ChromeMenuItem[];
};

export type ChromeMenuItem = {
  id: string;
  label: string;
  icon?: string;
  enabled?: boolean;
  selected?: boolean;
  children?: ChromeMenuItem[];
};

export type ContainerChrome = {
  visibility: "visible" | "hidden";
  title?: string;
  leading?: Array<{ id: string; actions: ChromeAction[] }>;
  trailing?: Array<{ id: string; actions: ChromeAction[] }>;
  drag_regions?: Array<{ x: number; y: number; width: number; height: number }>;
};

export type ContainerPage = {
  id: string;
  revision: number;
  surface_id: string;
  chrome: ContainerChrome;
  preferred_content_size: { width: number; height: number };
};

export type ContainerTransition = {
  style: "none" | "fade" | "slide";
  duration_ms: number;
  easing: "linear" | "ease-out" | "ease-in-out" | "spring";
};

export type ContainerActionEvent = {
  container_id: string;
  page_id: string;
  revision: number;
  action_id: string;
  menu_item_id?: string;
  source: "pointer" | "keyboard" | "menu";
};
export type ContainerLifecycleEvent = {
  container_id: string;
  page_id: string;
  revision: number;
  phase: "will-appear" | "did-appear" | "will-disappear" | "did-disappear";
};
export type ContainerGeometryEvent = {
  container_id: string;
  revision: number;
  origin: { x: number; y: number };
  content_size: { width: number; height: number };
};
export type ContainerAcknowledgementEvent = {
  acknowledgement: Record<string, unknown>;
  operation?: { kind: PendingOperation["kind"]; page?: ContainerPage };
};
export type ContainerCompatibilityEvent = {
  missing: ContainerExtension[];
  supported: ContainerExtension[];
};

type PendingOperation = {
  kind: "open" | "update" | "close";
  page?: ContainerPage;
  signature?: string;
};

type HostMessage = { type: string; payload?: Record<string, any> };
type OpenDescriptor = {
  contextId: string;
  contextRevision: number;
  slot: string;
  root: ContainerPage;
  transition?: ContainerTransition;
};

type Listener<T> = (event: T) => void;

export class ContainerController {
  private revision = 0;
  private sequence = 1;
  private supported = new Set<ContainerExtension>();
  private pending = new Map<string, PendingOperation>();
  private actionListeners = new Set<Listener<ContainerActionEvent>>();
  private lifecycleListeners = new Set<Listener<ContainerLifecycleEvent>>();
  private geometryListeners = new Set<Listener<ContainerGeometryEvent>>();
  private acknowledgementListeners = new Set<Listener<ContainerAcknowledgementEvent>>();
  private cancellationListeners = new Set<Listener<Record<string, unknown>>>();
  private compatibilityListeners = new Set<Listener<ContainerCompatibilityEvent>>();
  private lastOpen?: OpenDescriptor;
  private compatible = false;

  constructor(
    readonly containerId: string,
    private readonly sendEffect: (message: unknown) => void,
    private readonly requiredExtensions: ContainerExtension[] = ["chrome"],
  ) {}

  get currentRevision(): number {
    return this.revision;
  }

  supports(extension: ContainerExtension): boolean {
    return this.supported.has(extension);
  }

  get isCompatible(): boolean {
    return this.compatible;
  }

  onAction(listener: Listener<ContainerActionEvent>): () => void {
    this.actionListeners.add(listener);
    return () => this.actionListeners.delete(listener);
  }

  onLifecycle(listener: Listener<ContainerLifecycleEvent>): () => void {
    this.lifecycleListeners.add(listener);
    return () => this.lifecycleListeners.delete(listener);
  }

  onGeometry(listener: Listener<ContainerGeometryEvent>): () => void {
    this.geometryListeners.add(listener);
    return () => this.geometryListeners.delete(listener);
  }

  onAcknowledgement(listener: Listener<ContainerAcknowledgementEvent>): () => void {
    this.acknowledgementListeners.add(listener);
    return () => this.acknowledgementListeners.delete(listener);
  }

  onCancellation(listener: Listener<Record<string, unknown>>): () => void {
    this.cancellationListeners.add(listener);
    return () => this.cancellationListeners.delete(listener);
  }

  onCompatibility(listener: Listener<ContainerCompatibilityEvent>): () => void {
    this.compatibilityListeners.add(listener);
    return () => this.compatibilityListeners.delete(listener);
  }

  open(
    contextId: string,
    contextRevision: number,
    slot: string,
    root: ContainerPage,
    transition?: ContainerTransition,
  ): void {
    this.lastOpen = { contextId, contextRevision, slot, root, transition };
    if (!this.compatible) return;
    const requestId = this.requestId();
    this.pending.set(requestId, { kind: "open", page: root });
    this.sendEffect({
      type: "open-container",
      payload: {
        request_id: requestId,
        container_id: this.containerId,
        context_id: contextId,
        context_revision: contextRevision,
        slot,
        root: normalizedPage(root),
        transition,
      },
    });
  }

  update(page: ContainerPage, transition?: ContainerTransition): void {
    const signature = JSON.stringify(normalizedPage(page));
    const duplicate = [...this.pending.values()].some(
      (operation) => operation.kind === "update" && operation.signature === signature,
    );
    if (!duplicate) this.transact("update", { page }, page, transition, signature);
  }

  configure(page: ContainerPage, transition?: ContainerTransition): void {
    this.update(page, transition);
  }

  close(): void {
    if (!this.compatible || this.revision === 0) return;
    const requestId = this.requestId();
    this.pending.set(requestId, { kind: "close" });
    this.sendEffect({
      type: "close-container",
      payload: {
        request_id: requestId,
        container_id: this.containerId,
        expected_revision: this.revision,
      },
    });
  }

  replay(): void {
    const descriptor = this.lastOpen;
    if (!descriptor || !this.compatible) return;
    this.revision = 0;
    this.pending.clear();
    this.open(
      descriptor.contextId,
      descriptor.contextRevision,
      descriptor.slot,
      descriptor.root,
      descriptor.transition,
    );
  }

  handle(rawMessage: unknown): boolean {
    const message = rawMessage as HostMessage;
    if (!message || typeof message.type !== "string") return false;
    if (message.type === "start") {
      this.supported = new Set(
        (message.payload?.extensions ?? []).map((entry: any) => entry.extension),
      );
      const missing = this.requiredExtensions.filter((extension) => !this.supported.has(extension));
      this.compatible = missing.length === 0;
      if (missing.length > 0) {
        const event = { missing, supported: [...this.supported] };
        for (const listener of this.compatibilityListeners) listener(event);
      }
      return true;
    }
    if (message.type === "container-acknowledgement") {
      const acknowledgement = message.payload;
      if (acknowledgement?.container_id !== this.containerId) return false;
      const operation = this.pending.get(acknowledgement.request_id);
      this.pending.delete(acknowledgement.request_id);
      const applied = acknowledgement.result?.Ok;
      if (applied) this.revision = Math.max(this.revision, Number(applied.revision) || 0);
      for (const listener of this.acknowledgementListeners) {
        listener({ acknowledgement, operation });
      }
      return true;
    }
    if (message.type !== "container-event") return false;
    const event = message.payload;
    if (event?.container_id !== this.containerId) return false;
    if (event.type === "chrome-action") {
      for (const listener of this.actionListeners) listener(event);
    } else if (event.type === "page-lifecycle") {
      this.revision = Math.max(this.revision, Number(event.revision) || 0);
      for (const listener of this.lifecycleListeners) listener(event);
    } else if (event.type === "geometry-changed") {
      this.revision = Math.max(this.revision, Number(event.revision) || 0);
      for (const listener of this.geometryListeners) listener(event as ContainerGeometryEvent);
    } else if (event.type === "cancelled" || event.type === "dismissed") {
      this.pending.clear();
      this.revision = 0;
      for (const listener of this.cancellationListeners) listener(event);
    }
    return true;
  }

  resetRuntimeState(retainDescriptor = false): void {
    this.revision = 0;
    this.pending.clear();
    if (!retainDescriptor) this.lastOpen = undefined;
  }

  private transact(
    kind: PendingOperation["kind"],
    operation: Record<string, unknown>,
    page?: ContainerPage,
    transition?: ContainerTransition,
    signature?: string,
  ): void {
    if (!this.compatible || this.revision === 0) return;
    const requestId = this.requestId();
    this.pending.set(requestId, { kind, page, signature });
    this.sendEffect({
      type: "transact-container",
      payload: {
        request_id: requestId,
        container_id: this.containerId,
        expected_revision: this.revision,
        operation: { operation: kind, ...normalizeOperation(operation) },
        transition,
      },
    });
  }

  private requestId(): string {
    const value = this.sequence++;
    return `30000000-0000-4000-8000-${value.toString(16).padStart(12, "0")}`;
  }
}

function normalizeOperation(operation: Record<string, unknown>): Record<string, unknown> {
  if (operation.page) return { page: normalizedPage(operation.page as ContainerPage) };
  return operation;
}

function normalizedPage(page: ContainerPage): ContainerPage {
  return {
    ...page,
    chrome: {
      visibility: page.chrome.visibility,
      title: page.chrome.title,
      leading: page.chrome.leading ?? [],
      trailing: page.chrome.trailing ?? [],
      drag_regions: page.chrome.drag_regions ?? [],
    },
  };
}
