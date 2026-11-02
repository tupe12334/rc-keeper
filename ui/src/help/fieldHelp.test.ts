import { describe, expect, it } from "vitest";
import { allDocUrls, FIELD_HELP, OFFICIAL_DOC_HOSTS } from "./fieldHelp";

describe("fieldHelp docUrl", () => {
  it("every docUrl is https and points at an official host", () => {
    for (const url of allDocUrls()) {
      expect(url.startsWith("https://"), url).toBe(true);
      const { hostname, pathname } = new URL(url);
      expect(OFFICIAL_DOC_HOSTS, url).toContain(hostname);
      if (hostname === "github.com") {
        // Only this repo's own docs may use github.com.
        expect(pathname.startsWith("/tupe12334/rc-keeper/"), url).toBe(true);
      }
    }
  });

  it("every field has a title, text and at most three sentences", () => {
    for (const [key, help] of Object.entries(FIELD_HELP)) {
      expect(help.title.trim(), key).not.toBe("");
      const sentences = help.text.split(/(?<=[.!?])\s+/).filter(Boolean);
      expect(sentences.length, key).toBeGreaterThanOrEqual(1);
      expect(sentences.length, key).toBeLessThanOrEqual(3);
    }
  });
});
