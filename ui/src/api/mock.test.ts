import { describe, expect, it, vi } from "vitest";
import { mockApi } from "./mock";
import type { InstanceConfig } from "./types";

const cfg = (folder: string): InstanceConfig => ({
  id: "",
  folder,
  name: null,
  chrome: true,
  permissionMode: null,
  spawn: null,
  capacity: null,
  createSessionInDir: null,
  sessionNamePrefix: null,
  extraArgs: [],
  autoTrust: true,
  autoStart: false,
});

describe("mock api round-trip", () => {
  it("adds, lists, updates, and removes an instance", async () => {
    const id = await mockApi.addInstance(cfg("/tmp/one"));
    expect(id).not.toBe("");
    let list = await mockApi.listInstances();
    expect(list.map((v) => v.config.folder)).toContain("/tmp/one");

    await mockApi.updateInstance({ ...list.find((v) => v.config.id === id)!.config, name: "renamed" });
    list = await mockApi.listInstances();
    expect(list.find((v) => v.config.id === id)?.config.name).toBe("renamed");

    await mockApi.removeInstance(id);
    list = await mockApi.listInstances();
    expect(list.some((v) => v.config.id === id)).toBe(false);
  });

  it("start moves an instance to running after about a second", async () => {
    vi.useFakeTimers();
    try {
      const id = await mockApi.addInstance(cfg("/tmp/two"));
      await mockApi.startInstance(id);
      expect((await mockApi.listInstances()).find((v) => v.config.id === id)?.status.state).toBe("starting");
      vi.advanceTimersByTime(1000);
      expect((await mockApi.listInstances()).find((v) => v.config.id === id)?.status.state).toBe("running");
    } finally {
      vi.useRealTimers();
    }
  });
});
