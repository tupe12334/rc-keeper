import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FIELD_HELP, type FieldKey } from "@/help/fieldHelp";
import { emptyForm, formToConfig } from "@/lib/instanceConfig";
import { InstanceForm } from "./InstanceForm";
import { SettingsDialog } from "./SettingsDialog";

const FORM_FIELDS: FieldKey[] = [
  "folder",
  "name",
  "chrome",
  "permissionMode",
  "spawn",
  "capacity",
  "createSessionInDir",
  "sessionNamePrefix",
  "extraArgs",
  "autoTrust",
  "autoStart",
];

const infoButton = (field: FieldKey) => screen.getByRole("button", { name: `About ${FIELD_HELP[field].title}` });

afterEach(() => {
  vi.restoreAllMocks();
});

describe("info buttons", () => {
  it("every form input has exactly one info button", () => {
    render(<InstanceForm initial={null} onClose={() => {}} />);
    const inputs = [
      ...screen.getAllByRole("textbox"),
      ...screen.getAllByRole("spinbutton"),
      ...screen.getAllByRole("switch"),
      ...screen.getAllByRole("combobox"),
    ];
    const infoButtons = screen.getAllByRole("button", { name: /^About / });
    expect(inputs).toHaveLength(FORM_FIELDS.length);
    expect(infoButtons).toHaveLength(FORM_FIELDS.length);
    for (const field of FORM_FIELDS) expect(infoButton(field)).toBeInTheDocument();
  });

  it("settings dialog has an info button for each input", () => {
    render(<SettingsDialog onClose={() => {}} />);
    expect(infoButton("settings.claudeBin")).toBeInTheDocument();
    expect(infoButton("settings.autostart")).toBeInTheDocument();
  });
});

describe("popover", () => {
  it("opens with the help text and an official docs link", async () => {
    const user = userEvent.setup();
    render(<InstanceForm initial={null} onClose={() => {}} />);
    await user.click(infoButton("capacity"));

    const help = FIELD_HELP.capacity;
    expect(screen.getByText(help.text)).toBeInTheDocument();
    expect(screen.getByText(help.cliFlag!)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Official docs ↗" })).toHaveAttribute("href", help.docUrl);
  });

  it("opens the docs link with the external opener", async () => {
    const user = userEvent.setup();
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    render(<InstanceForm initial={null} onClose={() => {}} />);
    await user.click(infoButton("autoTrust"));
    await user.click(screen.getByRole("link", { name: "Official docs ↗" }));

    expect(open).toHaveBeenCalledWith(FIELD_HELP.autoTrust.docUrl, "_blank", "noopener,noreferrer");
  });

  it("shows the meaning of the selected select value", async () => {
    const user = userEvent.setup();
    const initial = formToConfig({ ...emptyForm(), folder: "/a", permissionMode: "plan" }, "id-1");
    render(<InstanceForm initial={initial} onClose={() => {}} />);
    await user.click(infoButton("permissionMode"));

    const popover = screen.getByText(FIELD_HELP.permissionMode.values!.plan.text).closest("p")!;
    expect(within(popover).getByText("plan:")).toBeInTheDocument();
  });
});
