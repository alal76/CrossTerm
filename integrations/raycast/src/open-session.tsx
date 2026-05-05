import { ActionPanel, Action, List, showHUD, open } from '@raycast/api';
import { useState, useEffect } from 'react';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

interface CrossTermSession {
  id: string;
  name: string;
  host: string;
  port: number;
  protocol: string;
}

function loadSessions(): CrossTermSession[] {
  // CrossTerm stores sessions in ~/.config/crossterm/sessions.json
  const sessionsPath = path.join(os.homedir(), '.config', 'crossterm', 'sessions.json');
  try {
    const raw = fs.readFileSync(sessionsPath, 'utf-8');
    const parsed = JSON.parse(raw);
    // Sessions may be nested under a profile key; handle both flat and nested
    if (Array.isArray(parsed)) return parsed;
    if (parsed.sessions && Array.isArray(parsed.sessions)) return parsed.sessions;
    return [];
  } catch {
    return [];
  }
}

function openInCrossTerm(session: CrossTermSession): void {
  const crossTermUrl = `crossterm://session/${session.id}`;
  open(crossTermUrl);
}

export default function Command() {
  const [sessions, setSessions] = useState<CrossTermSession[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loaded = loadSessions();
    setSessions(loaded);
    setIsLoading(false);
  }, []);

  return (
    <List isLoading={isLoading} searchBarPlaceholder="Search CrossTerm sessions...">
      {sessions.length === 0 && !isLoading ? (
        <List.EmptyView
          title="No Sessions Found"
          description="Open CrossTerm to create and save SSH sessions."
          actions={
            <ActionPanel>
              <Action title="Open CrossTerm" onAction={() => open('crossterm://')} />
            </ActionPanel>
          }
        />
      ) : (
        sessions.map(session => (
          <List.Item
            key={session.id}
            title={session.name}
            subtitle={`${session.host}:${session.port}`}
            accessories={[{ text: session.protocol.toUpperCase() }]}
            actions={
              <ActionPanel>
                <Action
                  title="Open Session"
                  onAction={() => {
                    openInCrossTerm(session);
                    showHUD(`Opening ${session.name} in CrossTerm`);
                  }}
                />
                <Action.CopyToClipboard
                  title="Copy Host"
                  content={session.host}
                />
              </ActionPanel>
            }
          />
        ))
      )}
    </List>
  );
}
