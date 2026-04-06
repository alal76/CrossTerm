export enum SessionType {
  SSH = "ssh",
  SFTP = "sftp",
  SCP = "scp",
  RDP = "rdp",
  VNC = "vnc",
  Telnet = "telnet",
  Serial = "serial",
  LocalShell = "local_shell",
  WSL = "wsl",
  CloudShell = "cloud_shell",
  WebConsole = "web_console",
  KubernetesExec = "kubernetes_exec",
  DockerExec = "docker_exec",
}

export enum ConnectionStatus {
  Connected = "connected",
  Disconnected = "disconnected",
  Connecting = "connecting",
  Idle = "idle",
}

export enum SidebarMode {
  Sessions = "sessions",
  Snippets = "snippets",
  Tunnels = "tunnels",
}

export enum BottomPanelMode {
  SFTP = "sftp",
  Snippets = "snippets",
  AuditLog = "audit_log",
  Search = "search",
}

export enum SplitDirection {
  Horizontal = "horizontal",
  Vertical = "vertical",
}

export enum ThemeVariant {
  Dark = "dark",
  Light = "light",
  System = "system",
}

export enum CredentialType {
  Password = "password",
  SSHKey = "ssh_key",
  Certificate = "certificate",
  APIToken = "api_token",
  CloudCredential = "cloud_credential",
  TOTPSeed = "totp_seed",
}

// --- Credential Interfaces ---

export interface PasswordCredential {
  id: string;
  type: CredentialType.Password;
  name: string;
  username: string;
  password: string;
  domain?: string;
}

export interface SSHKeyCredential {
  id: string;
  type: CredentialType.SSHKey;
  name: string;
  privateKey: string;
  passphrase?: string;
  publicKey?: string;
}

export interface CertificateCredential {
  id: string;
  type: CredentialType.Certificate;
  name: string;
  certificate: string;
  privateKey: string;
  format: "pem" | "pkcs12";
}

export interface APITokenCredential {
  id: string;
  type: CredentialType.APIToken;
  name: string;
  provider: string;
  token: string;
  expiryDate?: string;
}

export interface CloudCredential {
  id: string;
  type: CredentialType.CloudCredential;
  name: string;
  provider: "aws" | "azure" | "gcp";
  accessKey: string;
  secretKey: string;
  region?: string;
  profileName?: string;
}

export interface TOTPSeedCredential {
  id: string;
  type: CredentialType.TOTPSeed;
  name: string;
  secret: string;
  issuer: string;
  digits: number;
  period: number;
}

export type Credential =
  | PasswordCredential
  | SSHKeyCredential
  | CertificateCredential
  | APITokenCredential
  | CloudCredential
  | TOTPSeedCredential;

// --- Session ---

export interface SessionConnection {
  host: string;
  port: number;
  protocolOptions?: Record<string, unknown>;
}

export interface Session {
  id: string;
  name: string;
  type: SessionType;
  group: string;
  tags: string[];
  icon?: string;
  colorLabel?: string;
  credentialRef?: string;
  connection: SessionConnection;
  startupScript?: string;
  environmentVariables?: Record<string, string>;
  notes?: string;
  createdAt: string;
  updatedAt: string;
  lastConnectedAt?: string;
  autoReconnect: boolean;
  keepAliveIntervalSeconds: number;
}

// --- Profile ---

export interface Profile {
  id: string;
  name: string;
  avatar?: string;
  authMethod: "password" | "biometric" | "hardware_key" | "os_credential_store";
  createdAt: string;
}

// --- Tabs ---

export interface Tab {
  id: string;
  sessionId: string;
  title: string;
  sessionType: SessionType;
  status: ConnectionStatus;
  pinned: boolean;
  order: number;
}

// --- Split Panes ---

export interface SplitPaneLeaf {
  type: "leaf";
  tabId: string;
}

export interface SplitPaneContainer {
  type: "container";
  direction: SplitDirection;
  children: SplitPane[];
  sizes: number[];
}

export type SplitPane = SplitPaneLeaf | SplitPaneContainer;

// --- Theme ---

export interface ThemeTokens {
  "surface-primary": string;
  "surface-secondary": string;
  "surface-elevated": string;
  "surface-sunken": string;
  "surface-overlay": string;
  "text-primary": string;
  "text-secondary": string;
  "text-disabled": string;
  "text-inverse": string;
  "text-link": string;
  "border-default": string;
  "border-subtle": string;
  "border-strong": string;
  "border-focus": string;
  "interactive-default": string;
  "interactive-hover": string;
  "interactive-active": string;
  "interactive-disabled": string;
  "status-connected": string;
  "status-disconnected": string;
  "status-connecting": string;
  "status-idle": string;
  "accent-primary": string;
  "accent-secondary": string;
  "terminal-foreground": string;
  "terminal-background": string;
  "terminal-cursor": string;
  "terminal-selection": string;
  "terminal-ansi-0": string;
  "terminal-ansi-1": string;
  "terminal-ansi-2": string;
  "terminal-ansi-3": string;
  "terminal-ansi-4": string;
  "terminal-ansi-5": string;
  "terminal-ansi-6": string;
  "terminal-ansi-7": string;
  "terminal-ansi-8": string;
  "terminal-ansi-9": string;
  "terminal-ansi-10": string;
  "terminal-ansi-11": string;
  "terminal-ansi-12": string;
  "terminal-ansi-13": string;
  "terminal-ansi-14": string;
  "terminal-ansi-15": string;
  [key: string]: string;
}

export interface ThemeFile {
  schema_version: number;
  name: string;
  author: string;
  variant: ThemeVariant;
  tokens: ThemeTokens;
}

// --- Settings Types ---

export type BellStyle = "visual" | "audio" | "none";
export type CursorStyle = "block" | "underline" | "bar";
export type Breakpoint = "compact" | "medium" | "expanded" | "large";

// --- Terminal Instance ---

export interface TerminalInstance {
  id: string;
  sessionId: string;
  status: ConnectionStatus;
  cols: number;
  rows: number;
  title: string;
}

// --- Help System ---

export interface HelpArticle {
  slug: string;
  title: string;
  category: string;
  order: number;
  keywords: string[];
  body: string;
}

export interface KeyboardShortcut {
  id: string;
  keys: string;
  macKeys: string;
  label: string;
  category: string;
}

// --- Feature Tour ---

export interface TourStep {
  targetSelector: string;
  title: string;
  description: string;
  position: "top" | "bottom" | "left" | "right";
}

export interface TourDefinition {
  id: string;
  steps: TourStep[];
}

// --- Snippets ---

export interface Snippet {
  id: string;
  name: string;
  command: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}
