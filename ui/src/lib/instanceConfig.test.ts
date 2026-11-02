import { describe, expect, it } from "vitest";
import { configToForm, emptyForm, formToConfig, parseExtraArgs, validateForm } from "./instanceConfig";

describe("parseExtraArgs", () => {
  it("splits on runs of whitespace and drops empties", () => {
    expect(parseExtraArgs("  --verbose   --model  opus ")).toEqual(["--verbose", "--model", "opus"]);
    expect(parseExtraArgs("   ")).toEqual([]);
  });

  it("keeps quoted spans as one arg", () => {
    expect(parseExtraArgs(`--foo "a b" --bar 'c d'`)).toEqual(["--foo", "a b", "--bar", "c d"]);
  });

  it("round-trips through configToForm, including args with spaces", () => {
    const base = { ...emptyForm(), folder: "/a" };
    const cfg = formToConfig({ ...base, extraArgs: `--foo "a b" --x` }, "id");
    expect(cfg.extraArgs).toEqual(["--foo", "a b", "--x"]);
    const edited = configToForm(cfg);
    expect(edited.extraArgs).toBe(`--foo "a b" --x`);
    expect(formToConfig(edited, "id").extraArgs).toEqual(["--foo", "a b", "--x"]);
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
      forceBoot: false,
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

  it("caps capacity at u32::MAX", () => {
    const at = (capacity: string) => validateForm({ ...emptyForm(), folder: "/a", capacity });
    expect(at("4294967295")).toBeNull();
    expect(at("4294967296")).toMatch(/4294967295/);
    expect(at("1.5")).toMatch(/whole number/);
  });
});
