// ── CrossTerm MCP Coordinator Server ──
// Tracks gap analysis completion, manages work packages, and coordinates agents.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = resolve(__dirname, "../..");
const GAP_ANALYSIS_PATH = resolve(PROJECT_ROOT, "GAP-ANALYSIS.md");
const STATE_PATH = resolve(__dirname, "../.coordinator-state.json");

// ── Types ──

interface GapItem {
  id: string;
  description: string;
  specReference: string;
  severity: string;
  status: "Missing" | "In-Progress" | "Done" | "Stub";
  phase: number;
  category: string;
  assignedAgent?: string;
  completedAt?: string;
}

interface WorkPackage {
  id: string;
  name: string;
  description: string;
  phase: number;
  gapIds: string[];
  status: "pending" | "in-progress" | "completed";
  assignedAgent?: string;
  files: string[];
}

interface CoordinatorState {
  gaps: GapItem[];
  workPackages: WorkPackage[];
  lastUpdated: string;
}

// ── State Management ──

function loadState(): CoordinatorState {
  if (existsSync(STATE_PATH)) {
    return JSON.parse(readFileSync(STATE_PATH, "utf-8"));
  }
  return buildInitialState();
}

function saveState(state: CoordinatorState): void {
  state.lastUpdated = new Date().toISOString();
  writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
}

// ── Parse GAP-ANALYSIS.md ──

function parseGapAnalysis(): GapItem[] {
  const content = readFileSync(GAP_ANALYSIS_PATH, "utf-8");
  const gaps: GapItem[] = [];
  const lines = content.split("\n");

  let currentPhase = 1;
  let currentCategory = "";

  for (const line of lines) {
    // Detect phase
    if (line.includes("Phase 2")) currentPhase = 2;
    if (line.includes("Phase 3")) currentPhase = 3;

    // Detect category sections
    const sectionMatch = line.match(/^### \d+\.\d+ (.+)/);
    if (sectionMatch) {
      currentCategory = sectionMatch[1].trim();
    }

    // Parse gap table rows
    const rowMatch = line.match(
      /^\| ([\w-]+) \| (.+?) \| (§[\d.]+) \| (.+?) \| (Missing|✅ Done|Partial|Stub|In-Progress) \|$/
    );
    if (rowMatch) {
      gaps.push({
        id: rowMatch[1].trim(),
        description: rowMatch[2].trim(),
        specReference: rowMatch[3].trim(),
        severity: rowMatch[4].trim().replace(/\*\*/g, ""),
        status: rowMatch[5].trim().replace("✅ ", "") as GapItem["status"],
        phase: currentPhase,
        category: currentCategory,
      });
    }
  }
  return gaps;
}

function buildInitialState(): CoordinatorState {
  const gaps = parseGapAnalysis();

  // Define work packages
  const workPackages: WorkPackage[] = [
    {
      id: "WP-01",
      name: "Update Done P2 Items",
      description:
        "Mark the 13 deferred P2 items as Done in GAP-ANALYSIS since they were already implemented",
      phase: 2,
      gapIds: [
        "BE-SSH-10",
        "BE-VAULT-07",
        "BE-VAULT-08",
        "BE-VAULT-09",
        "FE-TAB-01",
        "FE-I18N-03",
        "FE-MISC-03",
        "FE-MISC-04",
        "HELP-22",
        "HELP-23",
        "BLD-05",
        "BLD-07",
      ],
      status: "pending",
      files: ["GAP-ANALYSIS.md"],
    },
    {
      id: "WP-02",
      name: "RDP Module",
      description:
        "FreeRDP FFI bindings, NLA, TLS, multi-monitor, clipboard, drive/audio redirection, gateway, recording",
      phase: 2,
      gapIds: Array.from({ length: 13 }, (_, i) => `P2-RDP-${String(i + 1).padStart(2, "0")}`),
      status: "pending",
      files: [
        "src-tauri/src/rdp/mod.rs",
        "src/components/RdpViewer/RdpViewer.tsx",
        "src/components/RdpViewer/RdpToolbar.tsx",
      ],
    },
    {
      id: "WP-03",
      name: "VNC Module",
      description:
        "VNC client with RFB 3.3/3.7/3.8, VeNCrypt TLS, encodings, clipboard, scaling",
      phase: 2,
      gapIds: Array.from({ length: 8 }, (_, i) => `P2-VNC-${String(i + 1).padStart(2, "0")}`),
      status: "pending",
      files: [
        "src-tauri/src/vnc/mod.rs",
        "src/components/VncViewer/VncViewer.tsx",
        "src/components/VncViewer/VncToolbar.tsx",
      ],
    },
    {
      id: "WP-04",
      name: "Cloud Module - AWS",
      description:
        "AWS CLI profile management, SSO, EC2 browser, SSM, S3, CloudWatch, ECS Exec, Lambda, Cost Dashboard",
      phase: 2,
      gapIds: [
        "P2-CLOUD-01",
        ...Array.from({ length: 9 }, (_, i) => `P2-CLOUD-${String(i + 2).padStart(2, "0")}`),
      ],
      status: "pending",
      files: ["src-tauri/src/cloud/mod.rs", "src-tauri/src/cloud/aws.rs"],
    },
    {
      id: "WP-05",
      name: "Cloud Module - Azure + GCP",
      description:
        "Azure CLI, AD login, VM browser, Bastion, Cloud Shell, Storage, AKS. GCP gcloud, IAP, Compute, GCS, Cloud Shell, GKE, Logging",
      phase: 2,
      gapIds: Array.from({ length: 15 }, (_, i) => `P2-CLOUD-${String(i + 11).padStart(2, "0")}`),
      status: "pending",
      files: ["src-tauri/src/cloud/azure.rs", "src-tauri/src/cloud/gcp.rs"],
    },
    {
      id: "WP-06",
      name: "Network Module",
      description:
        "Network scanner, reverse DNS, Wake-on-LAN, port forwarding manager, TFTP/HTTP server",
      phase: 2,
      gapIds: Array.from({ length: 8 }, (_, i) => `P2-NET-${String(i + 1).padStart(2, "0")}`),
      status: "pending",
      files: [
        "src-tauri/src/network/mod.rs",
        "src/components/NetworkTools/NetworkScanner.tsx",
        "src/components/NetworkTools/PortForwardManager.tsx",
        "src/components/NetworkTools/WakeOnLan.tsx",
      ],
    },
    {
      id: "WP-07",
      name: "Session Recording",
      description: "Asciicast v2 recording, playback with speed/seek, GIF/MP4 export",
      phase: 2,
      gapIds: ["P2-REC-01", "P2-REC-02", "P2-REC-03"],
      status: "pending",
      files: [
        "src-tauri/src/recording/mod.rs",
        "src/components/Recording/RecordingPlayer.tsx",
        "src/components/Recording/RecordingControls.tsx",
      ],
    },
    {
      id: "WP-08",
      name: "Cloud Dashboard Frontend",
      description:
        "Cloud dashboard component, provider panels, resource browsers, unified sidebar",
      phase: 2,
      gapIds: [],
      status: "pending",
      files: [
        "src/components/CloudDashboard/CloudDashboard.tsx",
        "src/components/CloudDashboard/AwsPanel.tsx",
        "src/components/CloudDashboard/AzurePanel.tsx",
        "src/components/CloudDashboard/GcpPanel.tsx",
        "src/components/CloudDashboard/CloudAssetTree.tsx",
      ],
    },
    {
      id: "WP-09",
      name: "Android Build & Frontend",
      description:
        "Android Tauri mobile target, extra-keys bar, bottom nav, session drawer, foreground service",
      phase: 2,
      gapIds: [],
      status: "pending",
      files: [],
    },
    {
      id: "WP-10",
      name: "Existing Modules P2 Additions",
      description:
        "Profile sync, FTP/FTPS, rsync-over-SSH, auto-lock vault, serial console, telnet, Kubernetes/Docker exec",
      phase: 2,
      gapIds: [],
      status: "pending",
      files: [
        "src-tauri/src/sync/mod.rs",
        "src-tauri/src/ftp/mod.rs",
        "src-tauri/src/serial/mod.rs",
        "src-tauri/src/telnet/mod.rs",
      ],
    },
    {
      id: "WP-11",
      name: "P2 Integration + Security + Build",
      description:
        "Cross-module integration, security hardening, build pipeline for all P2 features",
      phase: 2,
      gapIds: [],
      status: "pending",
      files: [".github/workflows/ci.yml"],
    },
    {
      id: "WP-12",
      name: "Plugin/WASM Runtime",
      description:
        "wasmtime-based plugin system, manifest, permissions, sandboxed FS/network, lifecycle hooks, KV store",
      phase: 3,
      gapIds: [],
      status: "pending",
      files: [
        "src-tauri/src/plugins/mod.rs",
        "src/components/Plugins/PluginManager.tsx",
        "src/components/Plugins/PluginSettings.tsx",
      ],
    },
    {
      id: "WP-13",
      name: "Macro & Expect Engine",
      description:
        "Macro recording/playback, expect-style YAML DSL, variable prompts, vault refs, broadcast execution",
      phase: 3,
      gapIds: [],
      status: "pending",
      files: [
        "src-tauri/src/macros/mod.rs",
        "src-tauri/src/expect/mod.rs",
        "src/components/Macros/MacroEditor.tsx",
        "src/components/Expect/ExpectEditor.tsx",
        "src/components/Expect/ExpectRunner.tsx",
      ],
    },
    {
      id: "WP-14",
      name: "Code Editor + Diff Viewer",
      description:
        "Monaco/CodeMirror embedded editor, syntax highlighting, SFTP save, side-by-side and inline diff",
      phase: 3,
      gapIds: [],
      status: "pending",
      files: [
        "src-tauri/src/editor/mod.rs",
        "src-tauri/src/diff/mod.rs",
        "src/components/CodeEditor/CodeEditor.tsx",
        "src/components/DiffViewer/DiffViewer.tsx",
      ],
    },
    {
      id: "WP-15",
      name: "SSH Key Manager + Localisation + Help P3",
      description:
        "SSH key manager enhancements, localisation framework, community locales, help system P3 additions",
      phase: 3,
      gapIds: [],
      status: "pending",
      files: [],
    },
    {
      id: "WP-16",
      name: "P3 Integration + Security + Build",
      description:
        "Plugin sandbox security, WASM signature verification, P3 build pipeline, CI for plugin tests",
      phase: 3,
      gapIds: [],
      status: "pending",
      files: [],
    },
    {
      id: "WP-17",
      name: "All-Phase Test Suite",
      description:
        "P2 tests (64 planned) + P3 tests (49 planned). Unit, integration, E2E, security, performance.",
      phase: 2,
      gapIds: [],
      status: "pending",
      files: [],
    },
  ];

  return { gaps, workPackages, lastUpdated: new Date().toISOString() };
}

// ── MCP Server ──

const server = new McpServer({
  name: "crossterm-coordinator",
  version: "1.0.0",
});

// ── Tool: Get Project Status ──

server.tool(
  "get_project_status",
  "Get overall project completion status across all phases",
  {},
  async () => {
    const state = loadState();
    const { gaps, workPackages } = state;

    const p1 = gaps.filter((g) => g.phase === 1);
    const p2 = gaps.filter((g) => g.phase === 2);
    const p3 = gaps.filter((g) => g.phase === 3);

    const countByStatus = (items: GapItem[]) => ({
      total: items.length,
      done: items.filter((g) => g.status === "Done").length,
      missing: items.filter((g) => g.status === "Missing").length,
      inProgress: items.filter((g) => g.status === "In-Progress").length,
      stub: items.filter((g) => g.status === "Stub").length,
    });

    const wpSummary = workPackages.map((wp) => ({
      id: wp.id,
      name: wp.name,
      status: wp.status,
      phase: wp.phase,
      gapCount: wp.gapIds.length,
      assignedAgent: wp.assignedAgent || "unassigned",
    }));

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              phase1: countByStatus(p1),
              phase2: countByStatus(p2),
              phase3: countByStatus(p3),
              overall: countByStatus(gaps),
              workPackages: wpSummary,
              lastUpdated: state.lastUpdated,
            },
            null,
            2
          ),
        },
      ],
    };
  }
);

// ── Tool: Get Work Package ──

server.tool(
  "get_work_package",
  "Get details of a specific work package including gap items and file targets",
  { packageId: z.string().describe("Work package ID (e.g., WP-02)") },
  async ({ packageId }) => {
    const state = loadState();
    const wp = state.workPackages.find((w) => w.id === packageId);
    if (!wp) {
      return {
        content: [{ type: "text" as const, text: `Work package ${packageId} not found` }],
      };
    }

    const relatedGaps = state.gaps.filter((g) => wp.gapIds.includes(g.id));

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify({ ...wp, gaps: relatedGaps }, null, 2),
        },
      ],
    };
  }
);

// ── Tool: List Pending Work ──

server.tool(
  "list_pending_work",
  "List all work packages that are not yet completed, optionally filtered by phase",
  { phase: z.number().optional().describe("Filter by phase (2 or 3)") },
  async ({ phase }) => {
    const state = loadState();
    let pending = state.workPackages.filter((wp) => wp.status !== "completed");
    if (phase) {
      pending = pending.filter((wp) => wp.phase === phase);
    }

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            pending.map((wp) => ({
              id: wp.id,
              name: wp.name,
              phase: wp.phase,
              status: wp.status,
              gapCount: wp.gapIds.length,
              files: wp.files,
            })),
            null,
            2
          ),
        },
      ],
    };
  }
);

// ── Tool: Assign Work Package ──

server.tool(
  "assign_work_package",
  "Assign a work package to an agent and mark it in-progress",
  {
    packageId: z.string().describe("Work package ID"),
    agentName: z.string().describe("Name of the agent assigned"),
  },
  async ({ packageId, agentName }) => {
    const state = loadState();
    const wp = state.workPackages.find((w) => w.id === packageId);
    if (!wp) {
      return {
        content: [{ type: "text" as const, text: `Work package ${packageId} not found` }],
      };
    }

    wp.status = "in-progress";
    wp.assignedAgent = agentName;
    saveState(state);

    return {
      content: [
        {
          type: "text" as const,
          text: `Assigned ${packageId} (${wp.name}) to agent "${agentName}". Status: in-progress.`,
        },
      ],
    };
  }
);

// ── Tool: Complete Work Package ──

server.tool(
  "complete_work_package",
  "Mark a work package as completed and update gap statuses",
  {
    packageId: z.string().describe("Work package ID"),
    gapStatuses: z
      .record(z.string(), z.enum(["Done", "Stub", "Missing"]))
      .optional()
      .describe("Override status for specific gap IDs"),
  },
  async ({ packageId, gapStatuses }) => {
    const state = loadState();
    const wp = state.workPackages.find((w) => w.id === packageId);
    if (!wp) {
      return {
        content: [{ type: "text" as const, text: `Work package ${packageId} not found` }],
      };
    }

    wp.status = "completed";

    // Update gap statuses
    if (gapStatuses) {
      for (const [gapId, status] of Object.entries(gapStatuses)) {
        const gap = state.gaps.find((g) => g.id === gapId);
        if (gap) {
          gap.status = status as GapItem["status"];
          if (status === "Done") gap.completedAt = new Date().toISOString();
        }
      }
    } else {
      // Default: mark all gaps in this package as Done
      for (const gapId of wp.gapIds) {
        const gap = state.gaps.find((g) => g.id === gapId);
        if (gap) {
          gap.status = "Done";
          gap.completedAt = new Date().toISOString();
        }
      }
    }

    saveState(state);

    return {
      content: [
        {
          type: "text" as const,
          text: `Work package ${packageId} (${wp.name}) marked as completed.`,
        },
      ],
    };
  }
);

// ── Tool: Get Gap Details ──

server.tool(
  "get_gap_details",
  "Get details of specific gap items by ID or filter by category/severity",
  {
    gapIds: z.array(z.string()).optional().describe("Specific gap IDs to retrieve"),
    category: z.string().optional().describe("Filter by category"),
    severity: z.string().optional().describe("Filter by severity"),
    status: z.string().optional().describe("Filter by status"),
    phase: z.number().optional().describe("Filter by phase"),
  },
  async ({ gapIds, category, severity, status, phase }) => {
    const state = loadState();
    let results = state.gaps;

    if (gapIds && gapIds.length > 0) {
      results = results.filter((g) => gapIds.includes(g.id));
    }
    if (category) {
      results = results.filter((g) =>
        g.category.toLowerCase().includes(category.toLowerCase())
      );
    }
    if (severity) {
      results = results.filter((g) =>
        g.severity.toLowerCase().includes(severity.toLowerCase())
      );
    }
    if (status) {
      results = results.filter((g) => g.status === status);
    }
    if (phase) {
      results = results.filter((g) => g.phase === phase);
    }

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(results, null, 2),
        },
      ],
    };
  }
);

// ── Tool: Update GAP-ANALYSIS Document ──

server.tool(
  "update_gap_document",
  "Update the GAP-ANALYSIS.md document to reflect current gap statuses from coordinator state",
  {},
  async () => {
    const state = loadState();
    let content = readFileSync(GAP_ANALYSIS_PATH, "utf-8");

    let updatedCount = 0;
    for (const gap of state.gaps) {
      if (gap.status === "Done" || gap.status === "Stub") {
        const statusText = gap.status === "Done" ? "✅ Done" : "⚠️ Stub";
        // Match the gap row and replace the status
        const pattern = new RegExp(
          `(\\| ${gap.id.replace(/[-]/g, "[-]")} \\|.+\\|)\\s*(Missing|Partial)\\s*\\|`,
          "g"
        );
        const newContent = content.replace(pattern, `$1 ${statusText} |`);
        if (newContent !== content) {
          content = newContent;
          updatedCount++;
        }
      }
    }

    if (updatedCount > 0) {
      writeFileSync(GAP_ANALYSIS_PATH, content);
    }

    return {
      content: [
        {
          type: "text" as const,
          text: `Updated ${updatedCount} gap statuses in GAP-ANALYSIS.md`,
        },
      ],
    };
  }
);

// ── Tool: Generate Agent Prompt ──

server.tool(
  "generate_agent_prompt",
  "Generate a detailed prompt for dispatching an agent to work on a specific work package",
  { packageId: z.string().describe("Work package ID") },
  async ({ packageId }) => {
    const state = loadState();
    const wp = state.workPackages.find((w) => w.id === packageId);
    if (!wp) {
      return {
        content: [{ type: "text" as const, text: `Work package ${packageId} not found` }],
      };
    }

    const relatedGaps = state.gaps.filter((g) => wp.gapIds.includes(g.id));

    const prompt = `
## Work Package: ${wp.id} — ${wp.name}

### Description
${wp.description}

### Phase
${wp.phase}

### Gap Items
${relatedGaps.map((g) => `- ${g.id}: ${g.description} (${g.severity}, ${g.specReference})`).join("\n")}

### Target Files
${wp.files.map((f) => `- ${f}`).join("\n")}

### Instructions
Implement all gap items listed above. Follow the project coding standards:
- Rust: thiserror enums, serde rename_all, Mutex for state, tauri::command pattern
- Frontend: React 18 function components, TypeScript strict, Tailwind, Zustand selectors
- Types in src/types/index.ts, i18n keys in src/i18n/en.json
- Tests: Rust #[cfg(test)] modules, Frontend vitest

Return a summary of all files created/modified and test results.
`;

    return {
      content: [{ type: "text" as const, text: prompt }],
    };
  }
);

// ── Start Server ──

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("CrossTerm Coordinator MCP server running on stdio");
}

main().catch(console.error);
