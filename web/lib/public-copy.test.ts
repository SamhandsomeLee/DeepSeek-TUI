import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

function pageSource(path: string): string {
  return readFileSync(new URL(`../app/[locale]/${path}`, import.meta.url), "utf8");
}

describe("public website copy contracts", () => {
  it("keeps the docs hub on the compact ocean portal instead of the old almanac treatment", () => {
    const layout = pageSource("docs/layout.tsx");
    const search = readFileSync(new URL("../components/docs-search.tsx", import.meta.url), "utf8");

    expect(layout).toContain("docs-portal-hero");
    expect(layout).toContain("Find the guidance you need.");
    expect(layout).not.toContain("Section 02");
    expect(layout).not.toContain("How Codewhale works: ego");
    expect(layout).not.toContain("<Seal");
    expect(search).toContain("docs-topic-row");
    expect(search).not.toContain("40+ Markdown documents");
  });

  it("does not rule out the managed app or make it a requirement for local use", () => {
    const roadmap = pageSource("roadmap/page.tsx");

    expect(roadmap).toContain("Managed app preview and optional accounts");
    expect(roadmap).toContain("Required account for the local runtime");
    expect(roadmap).not.toContain("Hosted SaaS dashboard");
    expect(roadmap).not.toContain("Required login / accounts");
  });

  it("describes ACP and the VS Code extension at their implemented capability level", () => {
    const runtime = pageSource("runtime/page.tsx");
    const sourceDocTargets = [
      ...new Set(
        [...runtime.matchAll(/REPO_BLOB_BASE}\/([^`]+)`/g)].map((match) => match[1]),
      ),
    ];

    expect(runtime).toContain("ACP (Agent Client Protocol)");
    expect(runtime).toContain("Baseline JSON-RPC adapter over stdio");
    expect(runtime).toContain("Phase 0 companion for the local runtime");
    expect(runtime).not.toContain("Agent Communication Protocol");
    expect(runtime).not.toContain("IETF-standard");
    expect(runtime).not.toContain("embeds Codewhale as a side-panel agent");
    expect(runtime).not.toMatch(/\/(?:en|zh)\/docs#(?:runtime-api|acp|mcp)/);
    expect(runtime).toContain("docs/RUNTIME_API.md");
    expect(runtime).toContain("docs/MCP.md");
    expect(sourceDocTargets).toEqual(["docs/RUNTIME_API.md", "docs/MCP.md"]);
    for (const target of sourceDocTargets) {
      expect(existsSync(new URL(`../../${target}`, import.meta.url)), target).toBe(true);
    }
  });

  it("uses the current modes, permission postures, and key guidance", () => {
    const modes = pageSource("docs/modes/page.tsx");
    const install = pageSource("install/page.tsx");
    const faq = pageSource("faq/page.tsx");
    const modeCopy = `${modes}\n${install}\n${faq}`;

    expect(modeCopy).not.toMatch(/\bAgent mode\b|Agent 模式|\bYOLO\b|suggest\s*\/\s*auto\s*\/\s*never|approval_mode|审批模式（建议/);
    for (const label of ["Plan", "Act", "Operate", "Ask", "Auto-Review", "Full Access"]) {
      expect(modes).toContain(label);
      expect(install).toContain(label);
    }
    expect(modes).toContain("/mode act");
    expect(modes).toContain("Shift+Tab");
    expect(modes).toContain("Plan is always Read Only");
    expect(modes).toContain("same permission posture, sandbox, and safety rules as Act");
    expect(faq).toContain("delegation is not mandatory");
    expect(modeCopy).not.toContain("executable work is dispatched to background Fleet workers");
    expect(install).toContain("New sessions open in Act mode by default");
  });

  it("presents providers as peers and puts contributor actions near the top", () => {
    const providerCopy = `${pageSource("models/page.tsx")}\n${pageSource("faq/page.tsx")}`;
    const community = pageSource("community/page.tsx");

    expect(providerCopy).not.toMatch(/first-class|一级支持|一级模型/);
    expect(community).toContain("International open-source community");
    expect(community).toContain("issues/new/choose");
    expect(community).toContain("docs/LOCALIZATION.md");
    expect(community).toContain("Hmbown/CodeWhale/pulls");
  });
});
