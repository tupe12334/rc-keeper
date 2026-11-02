import { describe, expect, it } from "vitest";
import { emptyForm, formToConfig, parseExtraArgs, validateForm } from "./instanceConfig";

describe("parseExtraArgs", () => {
  it("splits on runs of whitespace and drops empties", () => {
    expect(parseExtraArgs("  --verbose   --model  opus ")).toEqual(["--verbose", "--model", "opus"]);
    expect(parseExtraArgs("   ")).toEqual([]);
  });
});

describe("formToConfig", () => {
  it("maps unset selects to null and trims text", () => {
    const cfg = formToConfig({ ...emptyForm(), folder: " /a/b ", name: " app ", capacity: "2", extraArgs: "--x y" }, "id-1");
    expect(cfg).toEqual({
      id: "id-1",
      folder: "/a/b",
      name: "app",
      chrome: true,
      permissionMode: null,
      spawn: null,
      capacity: 2,
      createSessionInDir: null,
      sessionNamePrefix: null,
      extraArgs: ["--x", "y"],
      autoTrust: true,
      autoStart: true,
    });
  });

  it("maps explicit choices through", () => {
    const cfg = formToConfig({ ...emptyForm(), folder: "/a", permissionMode: "plan", spawn: "worktree", createSessionInDir: "off" }, "");
    expect(cfg.permissionMode).toBe("plan");
    expect(cfg.spawn).toBe("worktree");
    expect(cfg.createSessionInDir).toBe(false);
  });
});

describe("validateForm", () => {
  it("requires a folder and a positive whole capacity", () => {
    expect(validateForm(emptyForm())).toBe("Folder is required.");
    expect(validateForm({ ...emptyForm(), folder: "/a", capacity: "0" })).toMatch(/Capacity/);
    expect(validateForm({ ...emptyForm(), folder: "/a", capacity: "2" })).toBeNull();
  });
});
