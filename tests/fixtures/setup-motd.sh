#!/bin/bash
# Setup MOTD and SSH options for the test containers
# This script runs as a custom-cont-init script in linuxserver images

# Copy MOTD into place
cp /custom-motd /etc/motd 2>/dev/null || true

# Ensure sshd is configured for testing needs
# linuxserver images use /config/sshd/sshd_config (not /etc/ssh/sshd_config)
SSHD_CONFIG="/config/sshd/sshd_config"
if [ -f "$SSHD_CONFIG" ]; then
    sed -i 's/^AllowTcpForwarding no/AllowTcpForwarding yes/' "$SSHD_CONFIG"
    sed -i 's/^GatewayPorts no/GatewayPorts yes/' "$SSHD_CONFIG"
    sed -i 's/^X11Forwarding no/X11Forwarding yes/' "$SSHD_CONFIG"
    sed -i 's/^#PermitTunnel no/PermitTunnel yes/' "$SSHD_CONFIG"
    sed -i 's/^#PrintMotd yes/PrintMotd yes/' "$SSHD_CONFIG"
    # Uncomment Include directive so sshd_config.d/ snippets are loaded
    sed -i 's/^#Include \/etc\/ssh\/sshd_config\.d/Include \/etc\/ssh\/sshd_config.d/' "$SSHD_CONFIG"
fi

# Also patch /etc/ssh/sshd_config as a fallback
if [ -f /etc/ssh/sshd_config ]; then
    sed -i 's/^AllowTcpForwarding no/AllowTcpForwarding yes/' /etc/ssh/sshd_config
    sed -i 's/^GatewayPorts no/GatewayPorts yes/' /etc/ssh/sshd_config
fi

# Ensure /etc/pam.d/sshd has motd line (for PAM-based MOTD)
if [ -f /etc/pam.d/sshd ]; then
    grep -q "pam_motd" /etc/pam.d/sshd || \
        echo "session optional pam_motd.so" >> /etc/pam.d/sshd
fi

# Create a profile.d script as a fallback MOTD display mechanism
mkdir -p /etc/profile.d
cat > /etc/profile.d/motd.sh << 'EOPROFILE'
#!/bin/sh
[ -f /etc/motd ] && cat /etc/motd
EOPROFILE
chmod +x /etc/profile.d/motd.sh
