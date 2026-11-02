import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { State } from "@/api";
import { StatusBadge } from "./StatusBadge";
import { overallTone, stateLabel, stateTone } from "@/lib/status";

const cases: [State, string, string][] = [
  ["stopped", "Stopped", "gray"],
  ["starting", "Starting", "yellow"],
  ["running", "Running", "green"],
  ["restarting", "Restarting", "yellow"],
  ["folder-busy", "Folder busy", "yellow"],
  ["needs-trust", "Needs trust", "red"],
  ["error", "Error", "red"],
];

describe("state to badge mapping", () => {
  it.each(cases)("%s shows '%s' with tone %s", (state, text, tone) => {
    expect(stateLabel(state)).toBe(text);
    expect(stateTone(state)).toBe(tone);
    render(<StatusBadge state={state} />);
    expect(screen.getByText(text)).toBeInTheDocument();
  });

  it("shows the countdown while restarting", () => {
    expect(stateLabel("restarting", 3)).toBe("Restarting in 3s");
    render(<StatusBadge state="restarting" nextRetrySecs={3} />);
    expect(screen.getByText("Restarting in 3s")).toBeInTheDocument();
  });

  it("overall tone: red beats yellow, all running is green, none is gray", () => {
    const view = (state: State) => ({
      config: { id: state, folder: "/x", name: null, chrome: true, permissionMode: null, spawn: null, capacity: null, createSessionInDir: null, sessionNamePrefix: null, extraArgs: [], autoTrust: true, autoStart: true },
      status: { id: state, state, pid: null, sessions: 0, uptimeSecs: null, restarts: 0, nextRetrySecs: null, lastError: null, sessionUrls: [] },
    });
    expect(overallTone([])).toBe("gray");
    expect(overallTone([view("running"), view("running")])).toBe("green");
    expect(overallTone([view("running"), view("starting")])).toBe("yellow");
    expect(overallTone([view("running"), view("error")])).toBe("red");
    expect(overallTone([view("stopped")])).toBe("gray");
  });
});
