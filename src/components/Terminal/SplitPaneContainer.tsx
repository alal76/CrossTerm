import { useState, useCallback, useRef, useEffect } from "react";
import clsx from "clsx";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { SplitDirection, SessionType } from "@/types";
import type { SplitPane, SplitPaneLeaf, SplitPaneContainer as SplitPaneContainerType, Session } from "@/types";
import TerminalTab from "@/components/Terminal/TerminalTab";
import SshTerminalTab from "@/components/Terminal/SshTerminalTab";
import TelnetTerminal from "@/components/Telnet/TelnetTerminal";
import SerialTerminal from "@/components/Serial/SerialTerminal";
import {
  RdpTabPane,
  VncTabPane,
  WsTermTabPane,
  RedfishTabPane,
  WebDavTabPane,
  MqttTabPane,
  SmbTabPane,
  NetconfTabPane,
  MoshTabPane,
  WinRmTabPane,
  IpmiSolTabPane,
  SnmpTabPane,
  GrpcTabPane,
} from "@/components/Terminal/ProtocolTabPanes";

// ── Resize Handle ──

function ResizeHandle({
  direction,
  onDrag,
}: {
  readonly direction: SplitDirection;
  readonly onDrag: (delta: number) => void;
}) {
  const handleRef = useRef<HTMLButtonElement>(null);
  const dragging = useRef(false);
  const lastPos = useRef(0);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      lastPos.current = direction === SplitDirection.Horizontal ? e.clientX : e.clientY;

      const onMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const current = direction === SplitDirection.Horizontal ? ev.clientX : ev.clientY;
        const delta = current - lastPos.current;
        lastPos.current = current;
        onDrag(delta);
      };

      const onMouseUp = () => {
        dragging.current = false;
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.body.style.cursor = direction === SplitDirection.Horizontal ? "col-resize" : "row-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    },
    [direction, onDrag]
  );

  return (
    <button
      ref={handleRef}
      type="button"
      aria-label="Resize pane"
      onMouseDown={onMouseDown}
      className={clsx(
        "shrink-0 bg-border-subtle hover:bg-accent-primary transition-colors duration-[var(--duration-micro)] border-none p-0 outline-none",
        direction === SplitDirection.Horizontal
          ? "w-1 cursor-col-resize hover:w-1"
          : "h-1 cursor-row-resize hover:h-1"
      )}
    />
  );
}

// ── Leaf Renderer ──

// Self-contained viewers that own their own connect form and don't need a
// session's config passed in (Telnet/Serial).
const NO_PROPS_PANES: Partial<Record<SessionType, () => React.JSX.Element>> = {
  [SessionType.Telnet]: TelnetTerminal,
  [SessionType.Serial]: SerialTerminal,
};

// Protocol viewers that all take the same {sessionId, session} shape via
// the ProtocolTabPanes wrappers — keeping them in a lookup table (rather
// than a growing if-chain) keeps this dispatch flat as more protocols land.
const SESSION_TAB_PANES: Partial<Record<SessionType, typeof RdpTabPane>> = {
  [SessionType.RDP]: RdpTabPane,
  [SessionType.VNC]: VncTabPane,
  [SessionType.WebSocketTerminal]: WsTermTabPane,
  [SessionType.Redfish]: RedfishTabPane,
  [SessionType.WebDav]: WebDavTabPane,
  [SessionType.MqttClient]: MqttTabPane,
  [SessionType.Smb]: SmbTabPane,
  [SessionType.NetConf]: NetconfTabPane,
  [SessionType.Mosh]: MoshTabPane,
  [SessionType.WinRM]: WinRmTabPane,
  [SessionType.IpmiSol]: IpmiSolTabPane,
  [SessionType.Snmp]: SnmpTabPane,
  [SessionType.GrpcExplorer]: GrpcTabPane,
};

function renderPaneContent(tab: { sessionId: string; sessionType: SessionType }, session: Session | undefined, isActive: boolean) {
  if (tab.sessionType === SessionType.SSH && session) {
    const pw = (session.connection.protocolOptions?.["password"] as string) ?? "";
    return (
      <SshTerminalTab
        sessionId={tab.sessionId}
        isActive={isActive}
        host={session.connection.host}
        port={session.connection.port}
        username={(session.connection.protocolOptions?.["username"] as string) ?? ""}
        credentialRef={session.credentialRef}
        auth={pw ? { type: "password" as const, password: pw } : { type: "none" as const }}
      />
    );
  }

  const NoPropsPane = NO_PROPS_PANES[tab.sessionType];
  if (NoPropsPane) return <NoPropsPane />;

  const TabPane = SESSION_TAB_PANES[tab.sessionType];
  if (TabPane && session) return <TabPane sessionId={tab.sessionId} session={session} />;

  return <TerminalTab sessionId={tab.sessionId} isActive={isActive} />;
}

function LeafPane({
  pane,
  isActive,
}: {
  readonly pane: SplitPaneLeaf;
  readonly isActive: boolean;
}) {
  const openTabs = useSessionStore((s) => s.openTabs);
  const sessions = useSessionStore((s) => s.sessions);
  const activePaneId = useTerminalStore((s) => s.activePaneId);
  const setActivePaneId = useTerminalStore((s) => s.setActivePaneId);

  const isFocusedPane = activePaneId === pane.tabId;
  const tab = openTabs.find((t) => t.id === pane.tabId);
  if (!tab) {
    return (
      <div className="flex items-center justify-center h-full w-full bg-surface-sunken text-text-disabled text-xs">
        No tab
      </div>
    );
  }

  const session = sessions.find((s) => s.id === tab.sessionId);

  return (
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions
    <div
      className={clsx(
        "h-full w-full border-2 transition-colors duration-[var(--duration-micro)]",
        isFocusedPane ? "border-accent-primary" : "border-transparent",
      )}
      onClick={() => setActivePaneId(pane.tabId)}
    >
      {renderPaneContent(tab, session, isActive)}
    </div>
  );
}

// ── Container Renderer ──

function SplitContainer({
  pane,
  activeTabId,
}: {
  readonly pane: SplitPaneContainerType;
  readonly activeTabId: string | null;
}) {
  const [sizes, setSizes] = useState<number[]>(pane.sizes);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setSizes(pane.sizes);
  }, [pane.sizes]);

  const handleDrag = useCallback(
    (index: number, delta: number) => {
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const totalSize = pane.direction === SplitDirection.Horizontal ? rect.width : rect.height;
      if (totalSize === 0) return;

      const deltaPercent = (delta / totalSize) * 100;
      setSizes((prev) => {
        const next = [...prev];
        const minSize = 10; // minimum 10%
        const newLeft = next[index] + deltaPercent;
        const newRight = next[index + 1] - deltaPercent;
        if (newLeft < minSize || newRight < minSize) return prev;
        next[index] = newLeft;
        next[index + 1] = newRight;
        return next;
      });
    },
    [pane.direction]
  );

  const isHorizontal = pane.direction === SplitDirection.Horizontal;

  return (
    <div
      ref={containerRef}
      className={clsx("flex h-full w-full", isHorizontal ? "flex-row" : "flex-col")}
    >
      {pane.children.map((child, i) => {
        const childKey = child.type === "leaf" ? child.tabId : `split-${i}-${child.direction}`;
        return (
          <div key={childKey} className="flex min-w-0 min-h-0" style={{ flexBasis: `${sizes[i]}%`, flexGrow: 0, flexShrink: 0 }}>
            <SplitPaneRenderer pane={child} activeTabId={activeTabId} />
            {i < pane.children.length - 1 && (
              <ResizeHandle
                direction={pane.direction}
                onDrag={(delta) => handleDrag(i, delta)}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

// ── Dispatch Renderer ──

function SplitPaneRenderer({
  pane,
  activeTabId,
}: {
  readonly pane: SplitPane;
  readonly activeTabId: string | null;
}) {
  if (pane.type === "leaf") {
    return <LeafPane pane={pane} isActive={activeTabId === pane.tabId} />;
  }
  return <SplitContainer pane={pane} activeTabId={activeTabId} />;
}

// ── Pane ID collection helper ──

function collectLeafIds(pane: SplitPane): string[] {
  if (pane.type === "leaf") return [pane.tabId];
  return pane.children.flatMap(collectLeafIds);
}

// ── Main Export ──

export default function SplitPaneContainer({
  pane,
  activeTabId,
}: {
  readonly pane: SplitPane;
  readonly activeTabId: string | null;
}) {
  const activePaneId = useTerminalStore((s) => s.activePaneId);
  const setActivePaneId = useTerminalStore((s) => s.setActivePaneId);

  useEffect(() => {
    const leafIds = collectLeafIds(pane);
    if (leafIds.length <= 1) return;

    function handleKeyDown(e: KeyboardEvent) {
      if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;

      const arrows = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];
      if (!arrows.includes(e.key)) return;

      e.preventDefault();
      const ids = collectLeafIds(pane);
      const currentIdx = ids.indexOf(activePaneId ?? "");
      let nextIdx: number;

      if (e.key === "ArrowRight" || e.key === "ArrowDown") {
        nextIdx = currentIdx < ids.length - 1 ? currentIdx + 1 : 0;
      } else {
        nextIdx = currentIdx > 0 ? currentIdx - 1 : ids.length - 1;
      }

      setActivePaneId(ids[nextIdx]);
    }

    globalThis.addEventListener("keydown", handleKeyDown);
    return () => globalThis.removeEventListener("keydown", handleKeyDown);
  }, [pane, activePaneId, setActivePaneId]);

  return (
    <div className="h-full w-full bg-surface-sunken">
      <SplitPaneRenderer pane={pane} activeTabId={activeTabId} />
    </div>
  );
}
