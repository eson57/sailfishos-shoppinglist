#!/usr/bin/env bash

# Adapted from rockpool's deploy.sh, itself taken from:
# https://github.com/i-love-coffee-i-love-tea/harbour-radiox/blob/master/deploy-sailfish.sh

# Deploy an RPM package to a Sailfish OS phone and open the install dialog.
#
# Usage:
#   ./deploy.sh [path-to-rpm] [target-host]
#   ./deploy.sh [--rpm path] [--host host]
#
# Examples:
#   ./deploy.sh --host phone-usb
#   ./deploy.sh RPMS/<application>-0.1.0-1.aarch64.rpm phone-wifi
#   ./deploy.sh phone-usb
#   ./deploy.sh --host phone-usb --rpm RPMS/<application>-0.1.0-1.aarch64.rpm

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="${APP_NAME:-${SAILJAIL_APPLICATION:?not set, run inside the devenv shell}}"
readonly APP_NAME
readonly SSH_CONNECT_TIMEOUT=5

RPM_PATH=""
TARGET_HOST=""
REMOTE_UID=""
REMOTE_HOME=""
REMOTE_DEST=""

usage() {
    echo "Usage: $0 [path-to-rpm] [target-host]"
    echo "       $0 [--rpm path] [--host host]"
    echo ""
    echo "Defaults: newest RPM in RPMS/ or rpms/. The SSH host is required."
}

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --host)
                TARGET_HOST="$2"
                shift 2
                ;;
            --rpm)
                RPM_PATH="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            --*)
                echo "ERROR: Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [ -f "$1" ]; then
                    RPM_PATH="$1"
                else
                    TARGET_HOST="$1"
                fi
                shift
                ;;
        esac
    done
}

newest_rpm() {
    local dir="$1"
    ls -t "$dir"/*.rpm 2>/dev/null | head -n 1 || true
}

resolve_rpm() {
    if [ -z "$RPM_PATH" ]; then
        RPM_PATH="$(newest_rpm "$SCRIPT_DIR/RPMS")"
    fi
    if [ -z "$RPM_PATH" ]; then
        RPM_PATH="$(newest_rpm "$SCRIPT_DIR/rpms")"
    fi
    if [ -z "$RPM_PATH" ]; then
        RPM_PATH="$(newest_rpm "$SCRIPT_DIR")"
    fi

    if [ -z "$RPM_PATH" ] || [ ! -f "$RPM_PATH" ]; then
        echo "ERROR: No RPM package found to deploy."
        echo "Build one into RPMS/ first, or pass an RPM path."
        usage
        exit 1
    fi
}

resolve_target_host() {
    if [ -z "$TARGET_HOST" ]; then
        echo "ERROR: No SSH host given."
        echo "Pass a host from your SSH config, for example: ./deploy.sh phone-usb"
        usage
        exit 1
    fi
}

detect_remote() {
    echo "Checking connection to $TARGET_HOST..."
    local remote_info
    remote_info="$(ssh -o ConnectTimeout="$SSH_CONNECT_TIMEOUT" "$TARGET_HOST" 'id -u; echo "$HOME"' 2>/dev/null)" || {
        echo "ERROR: Failed to connect to $TARGET_HOST via SSH."
        echo "Please ensure the phone is connected, developer mode / SSH is active, and the host is reachable."
        exit 1
    }

    REMOTE_UID="$(echo "$remote_info" | sed -n '1p')"
    REMOTE_HOME="$(echo "$remote_info" | sed -n '2p')"

    if ! echo "$REMOTE_UID" | grep -qE '^[0-9]+$'; then
        echo "ERROR: Unexpected REMOTE_UID '$REMOTE_UID' — expected a numeric UID."
        echo "Raw remote info: $remote_info"
        exit 1
    fi
    if [ -z "$REMOTE_HOME" ] || [ "${REMOTE_HOME#/}" = "$REMOTE_HOME" ]; then
        echo "ERROR: Unexpected REMOTE_HOME '$REMOTE_HOME' — expected an absolute path starting with /."
        echo "Raw remote info: $remote_info"
        exit 1
    fi

    REMOTE_DEST="$REMOTE_HOME/Downloads/$(basename "$RPM_PATH")"
}

stop_remote_app() {
    echo "Stopping any running $APP_NAME processes on $TARGET_HOST..."
    ssh "$TARGET_HOST" "killall -9 $APP_NAME 2>/dev/null || true"
}

copy_rpm() {
    local remote_downloads
    remote_downloads="$(dirname "$REMOTE_DEST")"
    echo "Copying $(basename "$RPM_PATH") to $TARGET_HOST:$remote_downloads/..."
    ssh "$TARGET_HOST" "mkdir -p '$remote_downloads'"
    scp -C -p "$RPM_PATH" "$TARGET_HOST:$REMOTE_DEST"
}

trigger_install() {
    echo "Triggering package install dialog on device..."
    ssh "$TARGET_HOST" bash -s <<EOF
set -e
BUS="unix:path=/run/user/$REMOTE_UID/dbus/user_bus_socket"
if [ ! -e "/run/user/$REMOTE_UID/dbus/user_bus_socket" ]; then
    BUS="\$DBUS_SESSION_BUS_ADDRESS"
fi

# Open package install dialog via Sailfish OS file service
DBUS_SESSION_BUS_ADDRESS="\$BUS" gdbus call --session \\
    --dest org.sailfishos.fileservice \\
    --object-path / \\
    --method org.sailfishos.fileservice.openUrl \\
    "file://$REMOTE_DEST" >/dev/null 2>&1 || true
EOF
}

main() {
    parse_args "$@"
    resolve_rpm
    resolve_target_host

    echo "=== Deploying $(basename "$RPM_PATH") to $TARGET_HOST ==="

    detect_remote
    stop_remote_app
    copy_rpm
    trigger_install

    echo "=== Deployment complete! Check your phone to confirm installation. ==="
}

main "$@"
