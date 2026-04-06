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

// --- RDP Types ---

export interface RdpConfig {
  host: string;
  port: number;
  username: string;
  credential_ref?: string;
  domain?: string;
  nla_enabled: boolean;
  tls_required: boolean;
  gateway?: RdpGateway;
  multi_monitor?: RdpMonitorConfig;
  codec: RdpCodec;
  clipboard_sync: boolean;
  drive_paths: DriveMapping[];
  printer_redirect: boolean;
  audio_mode: RdpAudioMode;
  smart_card: boolean;
}

export type RdpCodec = "auto" | "remotefx" | "gfx" | "progressive";
export type RdpAudioMode = "none" | "playback" | "record" | "both";
export type RdpConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  | { error: string };

export interface RdpGateway {
  host: string;
  port: number;
  username: string;
  credential_ref?: string;
}

export interface RdpMonitorConfig {
  span_all: boolean;
  selected_monitors: number[];
}

export interface DriveMapping {
  name: string;
  local_path: string;
}

export interface RdpConnectionInfo {
  id: string;
  host: string;
  port: number;
  username: string;
  status: RdpConnectionStatus;
  width: number;
  height: number;
  connected_at?: string;
}

export interface RdpClipboardData {
  text?: string;
  files?: string[];
  image_png_base64?: string;
}

export interface RdpRedirectionConfig {
  drives: DriveMapping[];
  printer: boolean;
  audio: RdpAudioMode;
  smart_card: boolean;
}

export type RdpScaleMode = "fit" | "actual" | "fit-width" | "fit-height";

// --- VNC Types ---

export interface VncConfig {
  host: string;
  port: number;
  password?: string;
  vnc_auth: boolean;
  vencrypt: boolean;
  tls_cert_path?: string;
}

export type VncEncoding = 'raw' | 'copy_rect' | 'rre' | 'hextile' | 'zrle' | 'tight' | 'cursor_pseudo';
export type VncScalingMode = 'fit_to_window' | 'scroll' | 'one_to_one';
export type VncSecurityType = 'none' | 'vnc_auth' | 'vencrypt_tls' | 'vencrypt_x509';
export type VncConnectionStatus = 'connecting' | 'connected' | 'disconnected' | { error: string };

export interface VncConnectionInfo {
  id: string;
  host: string;
  port: number;
  status: VncConnectionStatus;
  width: number;
  height: number;
  view_only: boolean;
  scaling_mode: VncScalingMode;
}

// --- Cloud Types ---

export type CloudProvider = 'aws' | 'azure' | 'gcp';

export interface CloudProviderStatus {
  provider: CloudProvider;
  cli_status: CliStatus;
  profiles: string[];
  active_profile?: string;
}

export type CliStatus = { type: 'installed'; version: string; path: string } | { type: 'not_installed' };

export interface CloudAssetNode {
  id: string;
  name: string;
  node_type: CloudAssetType;
  provider: CloudProvider;
  children: CloudAssetNode[];
  metadata: Record<string, string>;
}

export type CloudAssetType = 'provider' | 'region' | 'resource_group' | 'compute' | 'storage' | 'kubernetes' | 'serverless' | 'database' | 'network';

export interface Ec2Instance {
  id: string;
  name: string;
  state: string;
  instance_type: string;
  public_ip?: string;
  private_ip?: string;
  key_name?: string;
  vpc_id?: string;
  launch_time: string;
}

export interface S3Bucket { name: string; region: string; creation_date: string; }
export interface S3Object { key: string; size: number; last_modified: string; storage_class: string; }

export interface AzureSubscription { id: string; name: string; state: string; tenant_id: string; }
export interface AzureVm { id: string; name: string; resource_group: string; location: string; status: string; public_ip?: string; private_ip?: string; size: string; }
export interface AzureStorageAccount { name: string; resource_group: string; kind: string; sku: string; location: string; }

export interface GcpConfig { name: string; project: string; region: string; zone: string; is_active: boolean; }
export interface GcpInstance { id: string; name: string; zone: string; machine_type: string; status: string; internal_ip?: string; external_ip?: string; }
export interface GcsBucket { name: string; location: string; storage_class: string; time_created: string; }
export interface GcsObject { name: string; size: number; content_type: string; time_created: string; }

export interface CostSummary { total_cost: number; currency: string; start_date: string; end_date: string; by_service: ServiceCost[]; }
export interface ServiceCost { service_name: string; cost: number; }

// --- Network Types ---

export interface ScanResult {
  ip: string;
  hostname?: string;
  mac_address?: string;
  open_ports: OpenPort[];
  os_guess?: string;
  response_time_ms: number;
}

export interface OpenPort {
  port: number;
  service_name: string;
  protocol: string;
}

export interface TunnelRule {
  id: string;
  name: string;
  local_port: number;
  remote_host: string;
  remote_port: number;
  tunnel_type: 'local' | 'remote' | 'dynamic';
  ssh_session_ref?: string;
  auto_start: boolean;
  enabled: boolean;
}

export type TunnelStatus = 'active' | 'inactive' | { error: string };

export interface FileServerInfo {
  id: string;
  directory: string;
  port: number;
  server_type: 'http' | 'tftp';
  running: boolean;
  url: string;
}

// --- Recording Types ---

export interface RecordingInfo {
  id: string;
  path: string;
  title?: string;
  duration_secs: number;
  size_bytes: number;
  width: number;
  height: number;
  created_at: string;
}

export interface PlaybackState {
  recording_id: string;
  position: number;
  speed: number;
  playing: boolean;
}

// --- Sync Types ---

export interface SyncStatus {
  last_export?: string;
  last_import?: string;
}

// --- FTP Types ---

export interface FtpConfig {
  host: string;
  port: number;
  username?: string;
  password?: string;
  use_tls: boolean;
  passive_mode: boolean;
}

export interface FtpEntry {
  name: string;
  size: number;
  entry_type: 'file' | 'directory' | 'link';
  modified?: string;
  permissions?: string;
}

// --- Serial Types ---

export interface SerialConfig {
  port_name: string;
  baud_rate: number;
  data_bits: 'five' | 'six' | 'seven' | 'eight';
  stop_bits: 'one' | 'two';
  parity: 'none' | 'odd' | 'even';
  flow_control: 'none' | 'software' | 'hardware';
}

export interface SerialPortInfo {
  name: string;
  description?: string;
  manufacturer?: string;
}

// --- Telnet Types ---

export interface TelnetConfig {
  host: string;
  port: number;
  terminal_type: string;
}
