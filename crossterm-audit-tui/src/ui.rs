//! Rendering. MC/nano-style: a title bar, a full-height body, and a
//! persistent function-key legend along the bottom — no free-scrolling
//! command-line interaction as the primary mode.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, Screen};

const LEGEND_STYLE: Style = Style::new().bg(Color::Blue).fg(Color::White);
const HEADER_STYLE: Style = Style::new()
    .fg(Color::Black)
    .bg(Color::Cyan)
    .add_modifier(Modifier::BOLD);
const SELECTED_STYLE: Style = Style::new().bg(Color::DarkGray).add_modifier(Modifier::BOLD);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    draw_title_bar(frame, chunks[0], app);

    draw_browser(frame, chunks[1], app);
    match &app.screen {
        Screen::Browser => {}
        Screen::Detail(idx) => draw_detail(frame, area, app, *idx),
        Screen::Help => draw_help(frame, area, app),
        Screen::QuitConfirm => draw_quit_confirm(frame, area),
    }

    draw_legend(frame, chunks[2], app);
}

fn draw_title_bar(frame: &mut Frame, area: Rect, app: &App) {
    let source = app
        .source_path
        .as_deref()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "(no file loaded)".to_string());
    let text = match &app.dump {
        Some(d) => format!(" crossterm-audit-tui — {source} — {} of {} host(s) shown", d.results.len(), d.host_count),
        None => format!(" crossterm-audit-tui — {source}"),
    };
    frame.render_widget(Paragraph::new(text).style(HEADER_STYLE), area);
}

fn draw_legend(frame: &mut Frame, area: Rect, app: &App) {
    let items: &[(&str, &str)] = match app.screen {
        Screen::Browser => &[("F1", "Help"), ("F3", "View"), ("F10", "Quit")],
        Screen::Detail(_) => &[("Esc", "Back")],
        Screen::Help => &[("Esc", "Back")],
        Screen::QuitConfirm => &[("Y", "Yes"), ("N/Esc", "No")],
    };
    let mut spans = Vec::new();
    for (key, label) in items {
        spans.push(Span::styled(format!(" {key} "), LEGEND_STYLE.add_modifier(Modifier::REVERSED)));
        spans.push(Span::styled(format!("{label} "), LEGEND_STYLE));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(LEGEND_STYLE), area);
}

fn draw_browser(frame: &mut Frame, area: Rect, app: &App) {
    let Some(dump) = &app.dump else {
        frame.render_widget(
            Paragraph::new(
                "No scan data loaded. Pass a network-explore-cli JSON file as an argument:\n\n  crossterm-audit-tui <path-to-scan.json>",
            )
            .block(Block::default().borders(Borders::ALL).title(" Host Browser ")),
            area,
        );
        return;
    };

    let header = Row::new(vec!["IP", "Hostname", "OS Guess", "Suggested Session", "Open Ports"]).style(HEADER_STYLE);
    let rows: Vec<Row> = dump
        .results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let style = if i == app.selected { SELECTED_STYLE } else { Style::default() };
            Row::new(vec![
                Cell::from(r.ip.clone()),
                Cell::from(r.hostname.clone().unwrap_or_else(|| "-".into())),
                Cell::from(r.os_guess.clone().unwrap_or_else(|| "-".into())),
                Cell::from(r.suggested_session_type.clone().unwrap_or_else(|| "-".into())),
                Cell::from(r.ports_summary()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(16),
        Constraint::Length(22),
        Constraint::Length(24),
        Constraint::Length(18),
        Constraint::Min(20),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(" Host Browser — {} ", dump.cidr)));
    frame.render_widget(table, area);
}

fn centered_rect(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App, idx: usize) {
    let Some(dump) = &app.dump else { return };
    let Some(host) = dump.results.get(idx) else { return };

    let popup = centered_rect(80, 80, area);
    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line> = vec![
        Line::from(format!("IP: {}", host.ip)),
        Line::from(format!("Hostname: {}", host.hostname.as_deref().unwrap_or("-"))),
        Line::from(format!("MAC: {} ({})", host.mac_address.as_deref().unwrap_or("-"), host.mac_vendor.as_deref().unwrap_or("unknown vendor"))),
        Line::from(format!("OS guess: {}", host.os_guess.as_deref().unwrap_or("-"))),
        Line::from(format!("TTL: {}", host.ttl.map(|t| t.to_string()).unwrap_or_else(|| "-".into()))),
        Line::from(format!(
            "Candidate sessions: {}",
            if host.candidate_session_types.is_empty() { "-".to_string() } else { host.candidate_session_types.join(", ") }
        )),
        Line::from(""),
        Line::from(Span::styled("Open ports:", Style::default().add_modifier(Modifier::BOLD))),
    ];
    if host.open_ports.is_empty() {
        lines.push(Line::from("  (none)"));
    }
    for p in &host.open_ports {
        lines.push(Line::from(format!("  {}/{} {}", p.port, p.protocol, p.service_name)));
        if let Some(b) = &p.banner {
            lines.push(Line::from(format!("    banner: {b}")));
        }
        if let Some(v) = &p.version {
            lines.push(Line::from(format!("    version: {v}")));
        }
        if let Some(t) = &p.http_title {
            lines.push(Line::from(format!("    title: {t}")));
        }
        if let Some(tls) = &p.tls {
            lines.push(Line::from(format!(
                "    tls: cn={} org={} issuer={} expires={}",
                tls.subject_cn.as_deref().unwrap_or("-"),
                tls.subject_org.as_deref().unwrap_or("-"),
                tls.issuer_org.as_deref().unwrap_or("-"),
                tls.not_after.as_deref().unwrap_or("-"),
            )));
            if !tls.san.is_empty() {
                lines.push(Line::from(format!("      SANs: {}", tls.san.join(", "))));
            }
        }
    }
    if !host.mdns.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("mDNS records:", Style::default().add_modifier(Modifier::BOLD))));
        for m in &host.mdns {
            let hostname = m.hostname.as_deref().unwrap_or("-");
            lines.push(Line::from(format!("  {} ({}), host={hostname}", m.instance_name, m.service_type)));
            for (k, v) in &m.txt {
                lines.push(Line::from(format!("    {k}={v}")));
            }
        }
    }
    if !host.evidence.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Evidence:", Style::default().add_modifier(Modifier::BOLD))));
        for e in &host.evidence {
            lines.push(Line::from(format!("  - {e}")));
        }
    }

    let block = Block::default().borders(Borders::ALL).title(format!(" Host Detail: {} ", host.ip));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup);
    let mut lines = vec![
        Line::from("crossterm-audit-tui — key bindings"),
        Line::from(""),
        Line::from("  Up/Down, j/k    Move selection"),
        Line::from("  Enter, F3       View host detail"),
        Line::from("  F1              This help screen"),
        Line::from("  F10, q          Quit"),
        Line::from("  Esc             Back / cancel"),
        Line::from(""),
        Line::from("See docs/network-explore-cli.md and"),
        Line::from("docs/network-audit-tui-plan.md for the full design."),
    ];
    if let Some(dump) = &app.dump {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Scan info:", Style::default().add_modifier(Modifier::BOLD))));
        lines.push(Line::from(format!("  CIDR: {}", dump.cidr)));
        lines.push(Line::from(format!("  Bound interface: {}", dump.bound_interface.as_deref().unwrap_or("-"))));
        lines.push(Line::from(format!(
            "  DNS servers used: {}",
            if dump.dns_servers_used.is_empty() { "-".to_string() } else { dump.dns_servers_used.join(", ") }
        )));
        lines.push(Line::from(format!(
            "  Ports scanned: {}",
            dump.ports_scanned.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
        )));
        if !dump.unmerged_mdns.is_empty() {
            lines.push(Line::from(format!(
                "  mDNS-only hosts (no open port/ping match): {}",
                dump.unmerged_mdns.keys().cloned().collect::<Vec<_>>().join(", ")
            )));
        }
    }
    let block = Block::default().borders(Borders::ALL).title(" Help ");
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_quit_confirm(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(30, 15, area);
    frame.render_widget(Clear, popup);
    let block = Block::default().borders(Borders::ALL).title(" Quit? ");
    frame.render_widget(Paragraph::new("Quit? [Y/n]").block(block), popup);
}
