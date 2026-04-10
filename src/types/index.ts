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
  NetworkExplorer = "network_explorer",
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
  Network = "network",
  RemoteFiles = "remote_files",
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

// --- Vault Metadata ---

export interface VaultInfo {
  id: string;
  name: string;
  is_default: boolean;
  owner_profile_id: string;
  shared_with: string[];
  created_at: string;
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
  group?: string;
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

// --- Network Explore Types ---

export type ServiceFilter =
  | 'ssh'
  | 'vnc'
  | 'rdp'
  | 'http'
  | 'https'
  | 'telnet'
  | 'ftp'
  | 'smb'
  | 'mysql'
  | 'postgresql'
  | 'redis'
  | 'mongodb'
  | { custom: number };

export interface ExploreTarget {
  cidr: string;
  services: ServiceFilter[];
  extra_ports: number[];
  timeout_ms?: number;
}

export interface ExploreResult {
  ip: string;
  hostname?: string;
  mac_address?: string;
  open_ports: OpenPort[];
  os_guess?: string;
  response_time_ms: number;
  suggested_session_type?: string;
}

export interface ExploreProgress {
  scan_id: string;
  hosts_scanned: number;
  total_hosts: number;
  hosts_found: number;
}

export interface ExploreHostFound {
  scan_id: string;
  result: ExploreResult;
}

// --- WiFi Scan Types ---

export type WifiBand = "band2_4ghz" | "band5ghz" | "band6ghz" | "unknown";

export type WifiSecurity =
  | "open"
  | "wep"
  | "wpa_psk"
  | "wpa2_psk"
  | "wpa3_sae"
  | "wpa3_transition"
  | "wpa2_enterprise"
  | "wpa3_enterprise"
  | { unknown: string };

export interface WifiNetwork {
  ssid: string;
  bssid?: string;
  channel: number;
  channel_width_mhz?: number;
  band: WifiBand;
  frequency_mhz?: number;
  signal_dbm?: number;
  noise_dbm?: number;
  security: WifiSecurity;
  phy_mode?: string;
  is_current: boolean;
}

export interface WifiSecurityIssue {
  ssid: string;
  severity: "critical" | "high" | "warning" | "info";
  issue: string;
  recommendation: string;
}

export interface WifiChannelCongestion {
  channel: number;
  band: WifiBand;
  network_count: number;
  strongest_signal_dbm?: number;
  weakest_signal_dbm?: number;
  congestion_level: "low" | "medium" | "high";
}

export interface WifiScanResult {
  networks: WifiNetwork[];
  security_issues: WifiSecurityIssue[];
  channel_congestion: WifiChannelCongestion[];
  recommended_channels_2g: number[];
  recommended_channels_5g: number[];
  current_network?: WifiNetwork;
  interface_name?: string;
  scan_timestamp: string;
}

// --- SSH Discovery Types ---

export interface SshDiscoverResult {
  host: string;
  port: number;
  none_auth_accepted: boolean;
  auth_required: boolean;
  banner?: string;
}

export interface SshConnectLogEvent {
  connection_id: string;
  level: "info" | "warn" | "error";
  message: string;
}

export interface SshBannerEvent {
  connection_id: string;
  banner: string;
}

export interface SshAuthSuccessEvent {
  connection_id: string;
  host: string;
  port: number;
  username: string;
  auth_method: string;
}

// --- Remote Monitoring Types ---

export interface RemoteStats {
  cpuPercent: number;
  memUsedMb: number;
  memTotalMb: number;
  diskUsedGb: number;
  diskTotalGb: number;
  loadAvg1: number;
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

// --- Plugin Types ---

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  permissions: PluginPermission[];
  entry_point: string;
  api_version: string;
}

export type PluginPermission = 'network' | 'file_system' | 'terminal' | 'clipboard' | 'notifications' | 'settings';

export interface PluginInfo {
  manifest: PluginManifest;
  enabled: boolean;
  loaded: boolean;
  load_time_ms?: number;
  error?: string;
}

// --- Macro Types ---

export type MacroStepType = 'send' | 'expect' | 'wait' | 'set_variable' | 'conditional' | 'loop';

export interface MacroStep {
  type: MacroStepType;
  data?: string;
  pattern?: string;
  timeout_ms?: number;
  duration_ms?: number;
  name?: string;
  value?: string;
  from_capture?: string;
  condition?: string;
  then_steps?: MacroStep[];
  else_steps?: MacroStep[];
  count?: number;
  steps?: MacroStep[];
}

export interface MacroInfo {
  id: string;
  name: string;
  description?: string;
  steps: MacroStep[];
  created_at: string;
  updated_at: string;
  tags: string[];
}

export type MacroExecutionStatus = 'pending' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';

export interface MacroExecution {
  id: string;
  macro_id: string;
  session_id: string;
  status: MacroExecutionStatus;
  current_step: number;
  total_steps: number;
  variables: Record<string, string>;
  started_at: string;
  completed_at?: string;
  error?: string;
}

export type ExpectActionType = 'send_text' | 'run_macro' | 'notify' | 'callback';

export interface ExpectRule {
  id: string;
  name: string;
  pattern: string;
  action: { type: ExpectActionType; text?: string; macro_id?: string; message?: string; event_name?: string };
  enabled: boolean;
}

// --- Editor Types ---

export interface EditorFile {
  id: string;
  path: string;
  content: string;
  encoding: string;
  language?: string;
  modified: boolean;
  line_count: number;
  size_bytes: number;
}

export interface DiffResult {
  left_path: string;
  right_path: string;
  hunks: DiffHunk[];
  stats: DiffStats;
}

export interface DiffHunk {
  left_start: number;
  left_count: number;
  right_start: number;
  right_count: number;
  lines: DiffLine[];
}

export interface DiffLine {
  line_type: 'context' | 'added' | 'removed';
  content: string;
  left_line?: number;
  right_line?: number;
}

export interface DiffStats {
  additions: number;
  deletions: number;
  modifications: number;
}

export interface SearchMatch {
  line: number;
  column: number;
  length: number;
  text: string;
}

// --- SSH Key Manager Types ---

export interface SshKeyInfo {
  id: string;
  name: string;
  key_type: string;
  fingerprint: string;
  public_key: string;
  private_key_path: string;
  comment?: string;
  created_at: string;
  last_used?: string;
  tags: string[];
}

export interface AgentKey {
  fingerprint: string;
  key_type: string;
  comment?: string;
  lifetime?: number;
}

export interface CertificateInfo {
  id: string;
  key_id: string;
  serial: number;
  cert_type: "user" | "host";
  valid_after: string;
  valid_before: string;
  principals: string[];
  extensions: string[];
}

// --- Localisation Types ---

export interface LocaleInfo {
  code: string;
  name: string;
  native_name: string;
  rtl: boolean;
  completeness: number;
}

// --- Security Types ---

export interface AuditEntry {
  id: string;
  timestamp: string;
  user: string;
  action: string;
  resource: string;
  details?: string;
  ip_address?: string;
  success: boolean;
}

export interface SecurityConfig {
  vault_timeout_secs: number;
  clipboard_clear_secs: number;
  audit_enabled: boolean;
  rate_limit: {
    max_attempts: number;
    window_secs: number;
    lockout_secs: number;
  };
}

export interface CertFingerprint {
  sha256: string;
  valid_from: string;
  valid_until: string;
  subject: string;
  pinned: boolean;
}

// --- SFTP Preview & Sync Types ---

export interface FilePreview {
  path: string;
  content_type: string;
  data: string;
  size: number;
  truncated: boolean;
}

export interface SyncEntry {
  path: string;
  local_modified?: string;
  remote_modified?: string;
  sync_action: 'upload' | 'download' | 'skip' | 'conflict';
  size: number;
}

export interface SyncResult {
  uploaded: number;
  downloaded: number;
  skipped: number;
  errors: string[];
}

// --- Azure Blob Types ---

export interface AzureBlobEntry {
  name: string;
  content_length: number;
  content_type: string;
  last_modified: string;
  blob_type: string;
}

// --- Plugin API Extensions ---

export type PluginHook = 'on_connect' | 'on_disconnect' | 'on_output_line' | 'on_command' | 'on_session_start' | 'on_session_end';

export interface PluginSandboxConfig {
  allowed_paths: string[];
  allowed_hosts: string[];
  max_memory_mb: number;
  max_cpu_time_ms: number;
}

export interface PluginRegistryEntry {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  downloads: number;
  category: string;
  installed: boolean;
  update_available: boolean;
}

// --- Android Types ---

export interface ForegroundServiceConfig {
  title: string;
  body: string;
  channel_id: string;
}

export interface NotificationChannel {
  id: string;
  name: string;
  description: string;
  importance: 'default' | 'high' | 'low' | 'min';
}
