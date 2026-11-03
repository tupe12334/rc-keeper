// Single source for every input's help text. Facts come from `claude remote-control --help`,
// `claude --help`, the official Claude Code docs, and rc-keeper's CONTRACT.md.

export interface HelpValue {
  text: string;
  docUrl?: string;
}

export interface FieldHelp {
  title: string;
  /** 1 to 3 short plain sentences: what it does, the default, when to change it. */
  text: string;
  docUrl: string;
  /** Exact `claude remote-control` flag this field maps to. Omit for rc-keeper-only settings. */
  cliFlag?: string;
  /** Meaning of each select value. Shown for the currently selected value. */
  values?: Record<string, HelpValue>;
}

export const OFFICIAL_DOC_HOSTS = ["code.claude.com", "github.com"] as const;

const REMOTE_CONTROL = "https://code.claude.com/docs/en/remote-control";
const PERMISSION_MODES = "https://code.claude.com/docs/en/permission-modes";
const CHROME = "https://code.claude.com/docs/en/chrome";
const WORKTREES = "https://code.claude.com/docs/en/worktrees";
const CLI_REFERENCE = "https://code.claude.com/docs/en/cli-reference";
const RC_KEEPER_CONTRACT = "https://github.com/tupe12334/rc-keeper/blob/main/CONTRACT.md";

export type FieldKey =
  | "folder"
  | "name"
  | "chrome"
  | "permissionMode"
  | "spawn"
  | "capacity"
  | "createSessionInDir"
  | "sessionNamePrefix"
  | "extraArgs"
  | "autoTrust"
  | "autoStart"
  | "forceBoot"
  | "settings.claudeBin"
  | "settings.autostart"
  | "settings.hideDock";

export const FIELD_HELP: Record<FieldKey, FieldHelp> = {
  folder: {
    title: "Folder",
    text: "The folder where Claude runs. Remote Control starts every session from this directory.",
    docUrl: REMOTE_CONTROL,
  },
  name: {
    title: "Name",
    text: "The session name shown on claude.ai/code. Empty means Claude picks a name for you.",
    docUrl: REMOTE_CONTROL,
    cliFlag: "--name",
  },
  chrome: {
    title: "Chrome",
    text: "Lets sessions in this folder use Claude in Chrome. The app always passes this flag. The default here is on.",
    docUrl: CHROME,
    cliFlag: "--chrome / --no-chrome",
  },
  permissionMode: {
    title: "Permission mode",
    text: "Sets which actions run without asking. Unset passes no flag, so Claude Code uses its own default.",
    docUrl: PERMISSION_MODES,
    cliFlag: "--permission-mode",
    values: {
      default: { text: "Manual. Reads run freely. Edits and commands ask first." },
      acceptEdits: { text: "Runs reads, file edits and common filesystem commands without asking." },
      auto: { text: "A classifier reviews each action and blocks the unsafe ones." },
      bypassPermissions: { text: "Runs everything without checks. Use only in isolated containers or VMs." },
      dontAsk: { text: "Runs pre-approved tools only. Denies anything that would ask. Suits CI." },
      plan: { text: "Explores and proposes a plan. Makes no edits until you approve." },
    },
  },
  spawn: {
    title: "Spawn mode",
    text: "Sets where each new session runs. Unset passes no flag, so same-dir applies unless a mode was saved for this folder.",
    docUrl: REMOTE_CONTROL,
    cliFlag: "--spawn",
    values: {
      "same-dir": { text: "All sessions share this folder. They can conflict when they edit the same files." },
      worktree: {
        text: "Each new session gets its own git worktree. The folder must be a git repository.",
        docUrl: WORKTREES,
      },
      session: { text: "One session only. The server exits when that session ends." },
    },
  },
  capacity: {
    title: "Capacity",
    text: "The most sessions that run at once. Unset uses 32. Not used with session spawn mode.",
    docUrl: REMOTE_CONTROL,
    cliFlag: "--capacity",
  },
  createSessionInDir: {
    title: "Create session in directory",
    text: "Starts one session in this folder when the server starts, so you can type at once. Unset uses the default, which is on.",
    docUrl: REMOTE_CONTROL,
    cliFlag: "--create-session-in-dir / --no-create-session-in-dir",
  },
  sessionNamePrefix: {
    title: "Session name prefix",
    text: "Prefix for auto-generated session names. Unset uses your computer's hostname. It applies only when no name is set.",
    docUrl: REMOTE_CONTROL,
    cliFlag: "--remote-control-session-name-prefix",
  },
  extraArgs: {
    title: "Extra arguments",
    text: "Extra flags passed to claude remote-control, separated by spaces. Quote a value that contains spaces.",
    docUrl: CLI_REFERENCE,
  },
  autoTrust: {
    title: "Trust folder automatically",
    text: "Answers the trust prompt for this folder when it starts. Off leaves the folder in Needs trust until you trust it in Claude Code.",
    docUrl: RC_KEEPER_CONTRACT,
  },
  forceBoot: {
    title: "Force boot",
    text: "When another claude remote-control serves this folder, stops it and starts this instance. Off waits, retries, and leaves the other one running.",
    docUrl: REMOTE_CONTROL,
  },
  autoStart: {
    title: "Start when the app opens",
    text: "Starts this folder when you add it and each time rc-keeper starts. Off keeps it stopped until you start it.",
    docUrl: RC_KEEPER_CONTRACT,
  },
  "settings.claudeBin": {
    title: "Claude binary path",
    text: "Full path to the claude program. Empty makes rc-keeper look on your PATH, then in ~/.local/bin.",
    docUrl: RC_KEEPER_CONTRACT,
  },
  "settings.autostart": {
    title: "Launch at login",
    text: "Opens rc-keeper when you sign in to your computer.",
    docUrl: RC_KEEPER_CONTRACT,
  },
  "settings.hideDock": {
    title: "Hide Dock icon",
    text: "macOS only. Removes rc-keeper from the Dock, so only the menu bar icon stays. Open the window from the menu bar icon.",
    docUrl: RC_KEEPER_CONTRACT,
  },
};

/** Every docUrl in FIELD_HELP, including per-value links. Used by tests. */
export function allDocUrls(): string[] {
  return Object.values(FIELD_HELP).flatMap((h) => [h.docUrl, ...Object.values(h.values ?? {}).map((v) => v.docUrl).filter((u): u is string => !!u)]);
}
