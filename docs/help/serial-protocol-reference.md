---
title: "Serial Protocol Reference"
slug: serial-protocol-reference
category: Protocols
keywords: [serial, baud, parity, flow control, RS-232, UART, break]
schema_version: 1
---

# Serial Protocol Reference

This reference covers serial port communication capabilities in CrossTerm, including baud rates, data framing, flow control, and break signal handling.

## Connection Parameters

### Baud Rate

The baud rate specifies the symbol rate in bits per second. CrossTerm supports standard and custom baud rates:

| Category | Rates |
|----------|-------|
| Low speed | 300, 1200, 2400, 4800 |
| Standard | 9600 (default), 19200, 38400 |
| High speed | 57600, 115200, 230400 |
| Very high speed | 460800, 921600, 1000000, 1500000, 2000000, 3000000, 4000000 |

Custom baud rates can be entered manually. Availability depends on the serial hardware and OS driver. Common embedded defaults: 9600 (Arduino), 115200 (ESP32, Raspberry Pi), 1500000 (some debug probes).

### Data Bits

The number of data bits per frame. Supported values:

| Value | Usage |
|-------|-------|
| 8 | Default. Standard for modern devices. |
| 7 | Legacy ASCII terminals, some industrial protocols (Modbus ASCII). |
| 6 | Rare. Historical teletypes. |
| 5 | Baudot code. Teletype (TTY) devices. |

### Stop Bits

Stop bits mark the end of each frame and provide time for the receiver to process the character.

| Value | Usage |
|-------|-------|
| 1 | Default. Sufficient for most applications. |
| 1.5 | Used with 5 data bits on some hardware. |
| 2 | Slower transmission. Provides more margin at low baud rates or long cables. |

### Parity

Parity provides basic single-bit error detection.

| Mode | Description |
|------|-------------|
| None | No parity bit. Default for most modern devices. |
| Odd | Parity bit set so total 1-bits (including parity) is odd. |
| Even | Parity bit set so total 1-bits (including parity) is even. |
| Mark | Parity bit always 1. Used for 9-bit addressing in multi-drop. |
| Space | Parity bit always 0. Rarely used. |

Common configurations: `9600 8N1` (9600 baud, 8 data bits, no parity, 1 stop bit) is the most widely used default. Industrial protocols often use `9600 7E1` (even parity) or `19200 8N1`.

## Flow Control

Flow control prevents data loss when the receiver cannot process data as fast as it arrives.

### Hardware Flow Control (RTS/CTS)

Uses dedicated RS-232 signal lines:

- **RTS (Request To Send)**: Asserted by the sender when ready to transmit.
- **CTS (Clear To Send)**: Asserted by the receiver when ready to accept data.

When CTS is deasserted, the sender pauses transmission until CTS is reasserted. This is the most reliable flow control method and is recommended for high baud rates and continuous data streams.

CrossTerm shows RTS/CTS line states in the status bar when hardware flow control is active.

### Software Flow Control (XON/XOFF)

Uses in-band control characters:

- **XOFF (0x13, Ctrl+S)**: Sent by receiver to pause transmission.
- **XON (0x11, Ctrl+Q)**: Sent by receiver to resume transmission.

Advantages: Works over 3-wire connections (TX, RX, GND) without hardware handshake lines.

Disadvantages: Control characters cannot appear in the data stream (problematic for binary data). Less responsive than hardware flow control due to propagation delay.

### DTR/DSR

An alternative hardware flow control using Data Terminal Ready (DTR) and Data Set Ready (DSR) lines. Less common than RTS/CTS. Some devices use DTR to signal power/presence rather than flow control.

### No Flow Control

No flow control. Suitable for low baud rates or when the application protocol handles its own flow control (e.g., line-by-line command/response protocols).

## Break Signal

The break signal is a special condition where the TX line is held in the spacing (low) state for longer than one frame duration. Uses:

- **Attention/interrupt**: Some devices use break as an attention signal (similar to Ctrl+C).
- **Magic SysRq**: Linux kernel triggers special debugging functions on serial break.
- **Cisco IOS**: Break signal enters ROM monitor mode on Cisco routers.

CrossTerm can send break via the terminal menu or keyboard shortcut. Break duration is configurable (default 250ms).

## RS-232 Signal Lines

CrossTerm displays the status of RS-232 modem control and status lines in the connection panel:

| Signal | Direction | Purpose |
|--------|-----------|---------|
| DTR | Output | Data Terminal Ready — terminal is powered and ready. |
| RTS | Output | Request To Send — terminal wants to send data. |
| DSR | Input | Data Set Ready — remote device is powered and ready. |
| CTS | Input | Clear To Send — remote device is ready to receive. |
| DCD | Input | Data Carrier Detect — carrier signal detected (modem). |
| RI | Input | Ring Indicator — incoming call (modem). |

## Line Endings

Serial terminals use different line ending conventions. CrossTerm supports configurable TX and RX line endings:

| Mode | TX sends | RX interprets |
|------|----------|---------------|
| CR | `\r` | Carriage return as newline |
| LF | `\n` | Line feed as newline |
| CRLF | `\r\n` | CR+LF as newline |

Default: send CR, receive CR or LF as newline. Configurable per-session.

## Hex View

CrossTerm provides a hex viewer mode for serial sessions, displaying received data as hexadecimal bytes alongside ASCII representation. Useful for debugging binary protocols, inspecting non-printable characters, and verifying framing parameters.

## Logging

Serial session data can be logged to a file in raw, hex, or timestamped formats. Log files capture all received data and optionally transmitted data. Timestamps use ISO 8601 format with millisecond precision.

## Troubleshooting

- **No data received**: Verify baud rate, data bits, parity, and stop bits match the device. Check cable connections (TX/RX may need to be crossed for DTE-to-DTE connections).
- **Garbled output**: Usually indicates mismatched baud rate. Try common rates: 9600, 115200.
- **Data loss at high rates**: Enable hardware flow control (RTS/CTS). Reduce baud rate if hardware doesn't support flow control.
- **Permission denied**: On Linux, add your user to the `dialout` group. On macOS, check System Preferences > Security for serial port access.
