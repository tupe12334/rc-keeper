import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api, type InstanceConfig } from "@/api";
import { BYPASS_WARNING, InstanceForm, TRUST_HELP } from "@/components/InstanceForm";

afterEach(() => vi.restoreAllMocks());

const baseCfg: InstanceConfig = {
  id: "x1",
  folder: "/Users/you/app",
  name: null,
  chrome: true,
  permissionMode: null,
  spawn: null,
  capacity: null,
  createSessionInDir: null,
  sessionNamePrefix: null,
  extraArgs: [],
  autoTrust: true,
  autoStart: true,
};

describe("App", () => {
  it("shows the empty state with an add button and the CLI tip", async () => {
    render(<App />);
    expect(await screen.findByText("No folders yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /add folder/i })).toBeInTheDocument();
    expect(screen.getByText(/rc-keeper/, { selector: "code" })).toBeInTheDocument();
  });

  it("add dialog builds the right InstanceConfig, parsing extra args", async () => {
    const add = vi.spyOn(api, "addInstance").mockResolvedValue("new-id");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /add folder/i }));

    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Folder"), "/Users/you/app");
    await user.type(within(dialog).getByLabelText("Name"), "app");
    await user.type(within(dialog).getByLabelText("Extra arguments"), "  --verbose   --model opus ");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(add).toHaveBeenCalledTimes(1);
    expect(add.mock.calls[0][0]).toEqual({
      id: "",
      folder: "/Users/you/app",
      name: "app",
      chrome: true,
      permissionMode: null,
      spawn: null,
      capacity: null,
      createSessionInDir: null,
      sessionNamePrefix: null,
      extraArgs: ["--verbose", "--model", "opus"],
      autoTrust: true,
      autoStart: true,
    });
  });

  it("requires a folder before saving", async () => {
    const add = vi.spyOn(api, "addInstance");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /add folder/i }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Save" }));
    expect(add).not.toHaveBeenCalled();
    expect(screen.getByText("Folder is required.")).toBeInTheDocument();
  });
});

describe("InstanceForm warnings", () => {
  it("shows the bypass warning only for bypassPermissions", () => {
    const { unmount } = render(<InstanceForm initial={{ ...baseCfg, permissionMode: "bypassPermissions" }} onClose={() => {}} />);
    expect(screen.getByRole("alert")).toHaveTextContent(BYPASS_WARNING);
    unmount();

    render(<InstanceForm initial={{ ...baseCfg, permissionMode: "acceptEdits" }} onClose={() => {}} />);
    expect(screen.queryByText(BYPASS_WARNING)).not.toBeInTheDocument();
  });

  it("shows the trust helper line under the trust switch", () => {
    render(<InstanceForm initial={baseCfg} onClose={() => {}} />);
    expect(screen.getByText(TRUST_HELP)).toBeInTheDocument();
  });
});
