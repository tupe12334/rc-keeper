import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";
import { api, type InstanceView } from "@/api";
import { TooltipProvider } from "@/components/ui/tooltip";
import { InstanceCard } from "./InstanceCard";

afterEach(() => vi.restoreAllMocks());

function view(state: InstanceView["status"]["state"]): InstanceView {
  return {
    config: {
      id: "app",
      folder: "/Users/you/app",
      name: "app",
      chrome: true,
      permissionMode: null,
      spawn: null,
      capacity: null,
      createSessionInDir: null,
      sessionNamePrefix: null,
      extraArgs: [],
      autoTrust: true,
      autoStart: true,
      forceBoot: false,
    },
    status: {
      id: "app",
      state,
      pid: null,
      sessions: 0,
      uptimeSecs: null,
      restarts: 0,
      nextRetrySecs: null,
      lastError: null,
      sessionUrls: [],
    },
  };
}

const card = (v: InstanceView) => (
  <TooltipProvider>
    <InstanceCard view={v} onEdit={() => {}} onLogs={() => {}} onChanged={() => {}} />
  </TooltipProvider>
);

describe("Take over", () => {
  it("shows the button only when the folder is busy", () => {
    const { unmount } = render(card(view("folder-busy")));
    expect(screen.getByRole("button", { name: "Take over" })).toBeInTheDocument();
    unmount();

    render(card(view("running")));
    expect(screen.queryByRole("button", { name: "Take over" })).not.toBeInTheDocument();
  });

  it("lists the pids from the preview and stops nothing until confirmed", async () => {
    const preview = vi.spyOn(api, "previewForceStart").mockResolvedValue({ stopped: [4321, 4322], respawnRisk: null });
    const force = vi.spyOn(api, "forceStart").mockResolvedValue({ stopped: [4321, 4322], respawnRisk: null });
    const success = vi.spyOn(toast, "success").mockReturnValue("");
    const user = userEvent.setup();
    render(card(view("folder-busy")));

    await user.click(screen.getByRole("button", { name: "Take over" }));
    const dialog = await screen.findByRole("dialog");
    expect(preview).toHaveBeenCalledWith("app");
    expect(within(dialog).getByText("PID 4321")).toBeInTheDocument();
    expect(within(dialog).getByText("PID 4322")).toBeInTheDocument();
    expect(force).not.toHaveBeenCalled();

    const confirm = within(dialog).getByRole("button", { name: "Take over folder" });
    expect(confirm).toHaveClass("bg-destructive");
    await user.click(confirm);
    expect(force).toHaveBeenCalledWith("app");
    expect(success).toHaveBeenCalledWith("Stopped PID 4321, PID 4322");
  });

  it("cancel closes the dialog and does not take over", async () => {
    vi.spyOn(api, "previewForceStart").mockResolvedValue({ stopped: [9], respawnRisk: null });
    const force = vi.spyOn(api, "forceStart");
    const user = userEvent.setup();
    render(card(view("folder-busy")));

    await user.click(screen.getByRole("button", { name: "Take over" }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
    expect(force).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("warns about the respawn risk", async () => {
    vi.spyOn(api, "previewForceStart").mockResolvedValue({ stopped: [77], respawnRisk: "launchd (ppid 1)" });
    const user = userEvent.setup();
    render(card(view("folder-busy")));

    await user.click(screen.getByRole("button", { name: "Take over" }));
    const alert = within(await screen.findByRole("dialog")).getByRole("alert");
    expect(alert).toHaveTextContent("Respawn risk: launchd (ppid 1)");
  });

  it("says nothing will stop when no foreign rc is found", async () => {
    vi.spyOn(api, "previewForceStart").mockResolvedValue({ stopped: [], respawnRisk: null });
    const user = userEvent.setup();
    render(card(view("folder-busy")));

    await user.click(screen.getByRole("button", { name: "Take over" }));
    expect(await screen.findByText(/Nothing will stop/)).toBeInTheDocument();
  });
});
