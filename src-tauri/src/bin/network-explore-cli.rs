//! Standalone Network Explorer runner — no Tauri app, no vault unlock, no UI
//! login. Runs the same discovery/enrichment pipeline as the app's Network
//! Explorer (`network_explore_start`), including TCP port scanning, ICMP
//! ping, ARP resolution, mDNS/Bonjour discovery, SNMP `sysDescr` probing,
//! IPMI presence detection, and OS/vendor guessing — and writes the full
//! result set as JSON.
//!
//! Full flag reference: run with `--help`, or see `docs/network-explore-cli.md`
//! for a walkthrough with examples and the JSON output schema.
//!
//! # Quick start
//! ```text
//! cargo run --release --bin network-explore-cli -- 192.168.1.0/24
//! cargo run --release --bin network-explore-cli -- 10.0.0.0/24 \
//!     --services ssh,winrm,ipmi --extra-ports 8006,9100 \
//!     --snmp-community private --table
//! ```
//!
//! # Relationship to the GUI's Network Explorer
//! This binary and `network_explore_start` (the Tauri command backing the
//! GUI) both call into the same probing primitives in `network::mod`, but
//! are two separate top-level functions: this one runs a single batch pass
//! and writes one JSON file with no progress reporting or cancellation,
//! while the GUI streams incremental progress/result events and supports
//! stopping a scan mid-flight. Discovery *capability* (which protocols get
//! probed, SNMP community handling, vendor hints) is kept in sync between
//! the two on a best-effort basis; if you find a gap, it's a bug — see
//! `run_explore_and_dump`'s doc comment in `network/mod.rs`.

use app_lib::network::{run_explore_and_dump, ServiceFilter};
use clap::Parser;
use std::time::Instant;

/// All named `ServiceFilter` variants this CLI can parse from `--services`,
/// paired with their default port — used for both parsing and `--list-services`.
/// Kept as a single source of truth here (rather than duplicated logic) by
/// parsing each name through `ServiceFilter`'s real `Deserialize` impl below,
/// so this list can never silently drift from what the enum actually accepts.
const SERVICE_NAMES: &[&str] = &[
    "ssh",
    "vnc",
    "rdp",
    "http",
    "https",
    "telnet",
    "ftp",
    "smb",
    "mysql",
    "postgresql",
    "redis",
    "mongodb",
    "win_rm",
    "win_rm_tls",
    "mqtt",
    "mqtt_tls",
    "netconf",
    "grpc",
    "kube_api",
    "docker_api",
    "docker_api_tls",
    "ws_terminal",
    "rtsp",
];

#[derive(Parser)]
#[command(
    name = "network-explore-cli",
    about = "Scan a CIDR range for reachable hosts and services, offline from the GUI app.",
    long_about = "Runs the same discovery pipeline as CrossTerm's Network Explorer (TCP port \
scan, ICMP ping, ARP, mDNS, SNMP, IPMI, OS/vendor guessing) against a CIDR range, with no Tauri \
app, vault, or UI required. Writes the full result set as JSON.\n\n\
See docs/network-explore-cli.md for the full output schema and worked examples."
)]
struct Args {
    /// CIDR range to scan, e.g. 192.168.1.0/24
    cidr: String,

    /// Output JSON file path
    #[arg(short, long, default_value = "network_scan.json")]
    out: String,

    /// Per-host TCP connect / ping timeout, in milliseconds. UDP probes
    /// (SNMP, IPMI) use a shorter internal timeout regardless — see
    /// docs/network-explore-cli.md.
    #[arg(short, long, default_value_t = 1500)]
    timeout_ms: u64,

    /// Comma-separated well-known services to scan. Defaults to the app's
    /// standard set (ssh, vnc, rdp, http, https, telnet, ftp, smb, mysql,
    /// postgresql, redis, mongodb, and the newer protocol-breadth additions).
    /// Run --list-services to see every valid name.
    #[arg(long, value_delimiter = ',')]
    services: Option<Vec<String>>,

    /// Additional arbitrary TCP ports to scan, beyond --services.
    #[arg(long, value_delimiter = ',')]
    extra_ports: Vec<u16>,

    /// Extra SNMP v1/v2c community strings to try against each host's
    /// UDP/161, in addition to "public" (always tried). Useful for managed
    /// switches/UPSes/printers deliberately configured off the default.
    #[arg(long, value_delimiter = ',')]
    snmp_community: Vec<String>,

    /// Extra vendor keywords (e.g. HIKVISION, MIKROTIK) to match against a
    /// discovered TLS certificate's subject/issuer organization, beyond the
    /// app's built-in vendor list.
    #[arg(long, value_delimiter = ',')]
    vendor_hint: Vec<String>,

    /// Print every valid --services name and exit without scanning.
    #[arg(long)]
    list_services: bool,

    /// Also print a human-readable summary table to stdout after scanning
    /// (the JSON file is always written regardless).
    #[arg(long)]
    table: bool,

    /// Suppress the "scanning..."/status lines normally printed to stderr.
    #[arg(short, long)]
    quiet: bool,
}

/// Parses one `--services` entry into a `ServiceFilter` by round-tripping it
/// through the enum's real `serde(rename_all = "snake_case")` `Deserialize`
/// impl, rather than hand-maintaining a second name mapping that could
/// silently drift from what `ServiceFilter` actually accepts.
fn parse_service(name: &str) -> Result<ServiceFilter, String> {
    let quoted = format!("\"{}\"", name.trim());
    serde_json::from_str(&quoted)
        .map_err(|_| format!("unknown service '{name}' (see --list-services)"))
}

fn print_table(out_path: &str) {
    let contents = match std::fs::read_to_string(out_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("--table: couldn't re-read {out_path}: {e}");
            return;
        }
    };
    let dump: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--table: couldn't parse {out_path} as JSON: {e}");
            return;
        }
    };
    let Some(results) = dump.get("results").and_then(|r| r.as_array()) else {
        return;
    };
    if results.is_empty() {
        println!("No hosts found.");
        return;
    }

    println!(
        "{:<16} {:<24} {:<20} {:<28} OPEN PORTS",
        "IP", "HOSTNAME", "OS GUESS", "SUGGESTED SESSION"
    );
    for r in results {
        let ip = r.get("ip").and_then(|v| v.as_str()).unwrap_or("?");
        let hostname = r.get("hostname").and_then(|v| v.as_str()).unwrap_or("-");
        let os_guess = r.get("os_guess").and_then(|v| v.as_str()).unwrap_or("-");
        let suggested = r
            .get("suggested_session_type")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let ports: String = r
            .get("open_ports")
            .and_then(|v| v.as_array())
            .map(|ports| {
                ports
                    .iter()
                    .filter_map(|p| {
                        let port = p.get("port")?.as_u64()?;
                        let svc = p.get("service_name")?.as_str()?;
                        Some(format!("{port}/{svc}"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        println!("{ip:<16} {hostname:<24} {os_guess:<20} {suggested:<28} {ports}");
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.list_services {
        println!("Valid --services names (default port in parentheses):");
        for name in SERVICE_NAMES {
            match parse_service(name) {
                Ok(svc) => println!("  {name} ({})", svc.port()),
                Err(e) => println!("  {name} (! {e})"),
            }
        }
        return;
    }

    let services: Option<Vec<ServiceFilter>> = match &args.services {
        None => None,
        Some(names) => {
            let mut parsed = Vec::with_capacity(names.len());
            for name in names {
                match parse_service(name) {
                    Ok(s) => parsed.push(s),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(2);
                    }
                }
            }
            Some(parsed)
        }
    };

    if !args.quiet {
        eprintln!(
            "scanning {} (timeout {}ms/probe{})...",
            args.cidr,
            args.timeout_ms,
            if args.extra_ports.is_empty() {
                String::new()
            } else {
                format!(", +{} extra port(s)", args.extra_ports.len())
            }
        );
    }

    let start = Instant::now();
    match run_explore_and_dump(
        &args.cidr,
        services.as_deref(),
        &args.extra_ports,
        args.timeout_ms,
        &args.out,
        &args.snmp_community,
        &args.vendor_hint,
    )
    .await
    {
        Ok(count) => {
            if !args.quiet {
                eprintln!(
                    "wrote {count} host(s) to {} in {:.1}s",
                    args.out,
                    start.elapsed().as_secs_f64()
                );
            }
            if args.table {
                print_table(&args.out);
            }
        }
        Err(e) => {
            eprintln!("scan failed: {e}");
            std::process::exit(1);
        }
    }
}
