import { describe, expect, test } from "bun:test";
import { ContainerController, type ContainerPage } from "./container";

const page: ContainerPage = {
  id: "40000000-0000-4000-8000-000000000001",
  revision: 1,
  surface_id: "10000000-0000-4000-8000-000000000001",
  chrome: { visibility: "visible", title: "Root" },
  preferred_content_size: { width: 320, height: 240 },
};

describe("ContainerController", () => {
  test("tracks authoritative revisions for one root page", () => {
    const effects: any[] = [];
    const controller = new ContainerController(
      "50000000-0000-4000-8000-000000000001",
      (effect) => effects.push(effect),
    );
    controller.handle({
      type: "start",
      payload: {
        extensions: [{ extension: "chrome", version: 1 }],
      },
    });
    expect(controller.supports("chrome")).toBe(true);
    controller.open("60000000-0000-4000-8000-000000000001", 1, "terminal-hover", page);
    const requestId = effects[0].payload.request_id;
    controller.handle({
      type: "container-acknowledgement",
      payload: {
        request_id: requestId,
        container_id: controller.containerId,
        result: { Ok: { revision: 1 } },
      },
    });
    controller.update({ ...page, revision: 2 });
    expect(effects.map((effect) => effect.type)).toEqual(["open-container", "transact-container"]);
  });

  test("keeps the newest authoritative revision when acknowledgements arrive out of order", () => {
    const effects: any[] = [];
    const controller = new ContainerController(
      "50000000-0000-4000-8000-000000000002",
      (effect) => effects.push(effect),
    );
    controller.handle({
      type: "start",
      payload: { extensions: [{ extension: "chrome", version: 1 }] },
    });
    controller.open("60000000-0000-4000-8000-000000000002", 1, "terminal-hover", page);
    const openRequest = effects[0].payload.request_id;
    controller.handle({
      type: "container-acknowledgement",
      payload: {
        request_id: openRequest,
        container_id: controller.containerId,
        result: { Ok: { revision: 4 } },
      },
    });
    controller.handle({
      type: "container-acknowledgement",
      payload: {
        request_id: "stale-response",
        container_id: controller.containerId,
        result: { Ok: { revision: 2 } },
      },
    });
    expect(controller.currentRevision).toBe(4);
  });

  test("reports compatibility failures and replays the last complete root state", () => {
    const effects: any[] = [];
    const compatibility: any[] = [];
    const controller = new ContainerController(
      "50000000-0000-4000-8000-000000000003",
      (effect) => effects.push(effect),
      ["chrome", "drag-regions"],
    );
    controller.onCompatibility((event) => compatibility.push(event));
    controller.handle({
      type: "start",
      payload: { extensions: [{ extension: "chrome", version: 1 }] },
    });
    controller.open("60000000-0000-4000-8000-000000000003", 7, "terminal-hover", page);
    expect(effects).toHaveLength(0);
    expect(compatibility[0].missing).toEqual(["drag-regions"]);

    controller.handle({
      type: "start",
      payload: {
        extensions: [
          { extension: "chrome", version: 1 },
          { extension: "drag-regions", version: 1 },
        ],
      },
    });
    controller.replay();
    expect(effects[0].payload.root.chrome.title).toBe("Root");
    expect(effects[0].payload.context_revision).toBe(7);
  });

  test("clears pending runtime state after host cancellation", () => {
    const effects: any[] = [];
    const cancellations: any[] = [];
    const controller = new ContainerController(
      "50000000-0000-4000-8000-000000000004",
      (effect) => effects.push(effect),
    );
    controller.onCancellation((event) => cancellations.push(event));
    controller.handle({
      type: "start",
      payload: { extensions: [{ extension: "chrome", version: 1 }] },
    });
    controller.open("60000000-0000-4000-8000-000000000004", 1, "terminal-hover", page);
    controller.handle({
      type: "container-event",
      payload: {
        type: "cancelled",
        container_id: controller.containerId,
        revision: 1,
        reason: "context closed",
      },
    });
    expect(controller.currentRevision).toBe(0);
    expect(cancellations).toHaveLength(1);
  });

  test("coalesces duplicate page updates but preserves semantic action details", () => {
    const effects: any[] = [];
    const actions: any[] = [];
    const controller = new ContainerController(
      "50000000-0000-4000-8000-000000000005",
      (effect) => effects.push(effect),
    );
    controller.onAction((event) => actions.push(event));
    controller.handle({
      type: "start",
      payload: { extensions: [{ extension: "chrome", version: 1 }] },
    });
    controller.open("60000000-0000-4000-8000-000000000005", 1, "terminal-hover", page);
    controller.handle({
      type: "container-acknowledgement",
      payload: {
        request_id: effects[0].payload.request_id,
        container_id: controller.containerId,
        result: { Ok: { revision: 1 } },
      },
    });
    const updated = { ...page, revision: 2, chrome: { ...page.chrome, title: "Updated" } };
    controller.update(updated);
    controller.update(updated);
    controller.handle({
      type: "container-event",
      payload: {
        type: "chrome-action",
        container_id: controller.containerId,
        page_id: page.id,
        revision: 1,
        action_id: "theme",
        menu_item_id: "dark",
        source: "menu",
      },
    });

    expect(effects.filter((effect) => effect.type === "transact-container")).toHaveLength(1);
    expect(actions[0].menu_item_id).toBe("dark");
  });
});
