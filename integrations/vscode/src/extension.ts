import * as vscode from 'vscode';
import { execSync } from 'child_process';
import * as os from 'os';
import * as path from 'path';

function getCrossTermSessions(): Array<{ id: string; name: string; host: string }> {
  // CrossTerm stores sessions in its config directory
  // This is a stub — real implementation reads from CrossTerm's config store
  const configDir = path.join(os.homedir(), '.config', 'crossterm');
  try {
    const sessionsFile = path.join(configDir, 'sessions.json');
    // In a real implementation, we'd read and parse sessions.json
    // Return empty for now
    return [];
  } catch {
    return [];
  }
}

function openCrossTermSession(sessionId: string): void {
  // Open CrossTerm with a specific session ID via CLI argument
  // The CrossTerm binary supports: crossterm --session <id>
  const crossTermBin = process.platform === 'darwin'
    ? '/Applications/CrossTerm.app/Contents/MacOS/CrossTerm'
    : process.platform === 'win32'
    ? 'CrossTerm.exe'
    : 'crossterm';

  try {
    execSync(`"${crossTermBin}" --session "${sessionId}"`, { detached: true, stdio: 'ignore' });
  } catch {
    vscode.window.showErrorMessage('CrossTerm not found. Please install CrossTerm from https://crossterm.app');
  }
}

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('crossterm.openSession', async () => {
      const sessions = getCrossTermSessions();
      if (sessions.length === 0) {
        const result = await vscode.window.showInformationMessage(
          'No CrossTerm sessions found. Open CrossTerm to create sessions.',
          'Open CrossTerm'
        );
        if (result === 'Open CrossTerm') {
          openCrossTermSession('');
        }
        return;
      }
      const picked = await vscode.window.showQuickPick(
        sessions.map(s => ({ label: s.name, description: s.host, sessionId: s.id })),
        { placeHolder: 'Select a CrossTerm session to open' }
      );
      if (picked) {
        openCrossTermSession(picked.sessionId);
      }
    }),

    vscode.commands.registerCommand('crossterm.openSFTP', async () => {
      const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
      if (!workspaceFolder) {
        vscode.window.showErrorMessage('No workspace folder open.');
        return;
      }
      vscode.window.showInformationMessage(`Opening SFTP browser for ${workspaceFolder.name} in CrossTerm...`);
      openCrossTermSession(`sftp:${workspaceFolder.uri.fsPath}`);
    }),

    vscode.commands.registerCommand('crossterm.listSessions', async () => {
      const sessions = getCrossTermSessions();
      const panel = vscode.window.createWebviewPanel(
        'crossTermSessions',
        'CrossTerm Sessions',
        vscode.ViewColumn.One,
        {}
      );
      panel.webview.html = `<html><body><h2>CrossTerm Sessions</h2><p>${sessions.length} sessions found.</p></body></html>`;
    })
  );
}

export function deactivate(): void {}
