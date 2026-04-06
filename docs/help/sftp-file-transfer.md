---
title: "SFTP File Transfer"
slug: "sftp-file-transfer"
category: "connections"
order: 3
schema_version: 1
keywords: ["sftp", "file", "transfer", "upload", "download", "browse", "drag", "drop", "queue", "remote"]
---

# SFTP File Transfer

CrossTerm includes a built-in SFTP browser for transferring files between your local machine and remote servers over SSH.

## Opening the SFTP Browser

There are several ways to access the SFTP browser:

1. **New tab**: Click **+** → **New SFTP Browser**, then connect to a host.
2. **Bottom panel**: When connected to an SSH session, open the bottom panel (**Ctrl+J** / **⌘J**) and select the **SFTP** tab.
3. **From an SSH session**: The SFTP browser automatically uses the same connection credentials.

## Browsing Remote Files

The SFTP browser provides a dual-pane interface:

- **Left pane**: Local file system.
- **Right pane**: Remote file system.

Navigate directories by double-clicking folders. The path bar at the top shows the current location and supports direct path entry.

### File Information

For each file and directory, you can see:

- File name and icon
- File size (human-readable)
- Last modified date
- Unix permissions

## Uploading Files

### Drag and Drop

The easiest way to upload files:

1. Open the SFTP browser.
2. Drag files from your desktop or file manager.
3. Drop them onto the remote pane.
4. A progress indicator shows the transfer status.

### Upload Button

1. Click the **Upload Files** button in the toolbar.
2. Select files from the file picker dialog.
3. Files are uploaded to the current remote directory.

### Multiple Files

Select multiple files to upload them in a batch. CrossTerm handles transfers concurrently for better performance.

## Downloading Files

1. Select one or more files in the remote pane.
2. Click **Download** or drag them to the local pane.
3. Choose a local destination if prompted.
4. Files are downloaded to the selected directory.

## Transfer Queue

When transferring multiple files, CrossTerm maintains a transfer queue:

- View active and pending transfers in the bottom panel.
- Each transfer shows progress percentage and speed.
- You can pause, resume, or cancel individual transfers.
- Failed transfers can be retried.

## File Operations

Right-click files or directories for additional operations:

- **Rename**: Change the file or directory name.
- **Delete**: Remove the file or directory (with confirmation).
- **Change permissions**: Modify Unix file permissions.
- **Create directory**: Create a new folder in the current location.

## Keyboard Navigation

- **Arrow keys**: Navigate the file list.
- **Enter**: Open directory or download file.
- **Delete/Backspace**: Delete selected files (with confirmation).
- **Ctrl+A / ⌘A**: Select all files.

## Performance Tips

- Large file transfers benefit from a stable connection with adequate bandwidth.
- Uploading many small files is slower than a few large files due to per-file overhead.
- Use compression in SSH settings for text-heavy transfers over slow connections.

## Security

All SFTP transfers are encrypted through the SSH tunnel. No data is transmitted in plaintext. File permissions and ownership are preserved during transfers when possible.
