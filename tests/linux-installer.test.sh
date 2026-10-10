#!/usr/bin/env bash
set -euo pipefail

ROOT="${INSTALLER_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TEMP_DIR="$(mktemp -d)"
FAKE_BIN="$TEMP_DIR/bin"
PLUGIN_GUID="{BE5CBF95-C0AD-4842-B157-AC40FEDD9441}"
trap 'rm -rf "$TEMP_DIR"' EXIT
mkdir -p "$FAKE_BIN"

cat > "$FAKE_BIN/systemctl" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
cat > "$FAKE_BIN/python3" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$FAKE_BIN/systemctl" "$FAKE_BIN/python3"

install_to() {
    local home="$1" data_home="$2" override_dir="$3" expected_dir="$4"
    mkdir -p "$home"
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$data_home" \
        ONLYOFFICE_PLUGIN_DIR="$override_dir" PATH="$FAKE_BIN:$PATH" \
        bash "$ROOT/install.sh" >/dev/null
    [[ -x "$home/.local/bin/mendeley-loopback-server" ]]
    cmp -s "$ROOT/scripts/mendeley-loopback-server.py" "$home/.local/bin/mendeley-loopback-server"
    [[ -f "$expected_dir/$PLUGIN_GUID/config.json" ]]
    [[ "$(readlink "$expected_dir/mendeley")" == "$PLUGIN_GUID" ]]
}

native_home="$TEMP_DIR/native-home"
native_data="$native_home/.local/share"
native_plugins="$native_data/onlyoffice/desktopeditors/sdkjs-plugins"
mkdir -p "$(dirname "$native_plugins")"
install_to "$native_home" "$native_data" "" "$native_plugins"

flatpak_home="$TEMP_DIR/flatpak-home"
flatpak_plugins="$flatpak_home/.var/app/org.onlyoffice.desktopeditors/data/onlyoffice/desktopeditors/sdkjs-plugins"
mkdir -p "$(dirname "$flatpak_plugins")"
install_to "$flatpak_home" "$flatpak_home/.local/share" "" "$flatpak_plugins"

snap_home="$TEMP_DIR/snap-home"
snap_plugins="$snap_home/snap/onlyoffice-desktopeditors/current/.local/share/onlyoffice/desktopeditors/sdkjs-plugins"
mkdir -p "$(dirname "$snap_plugins")"
install_to "$snap_home" "$snap_home/.local/share" "" "$snap_plugins"

custom_home="$TEMP_DIR/custom-home"
custom_plugins="$custom_home/editor/plugins/sdkjs-plugins"
install_to "$custom_home" "$custom_home/data" "$custom_plugins" "$custom_plugins"

missing_home="$TEMP_DIR/missing-home"
mkdir -p "$missing_home"
if env HOME="$missing_home" XDG_DATA_HOME="$missing_home/data" PATH="$FAKE_BIN:$PATH" \
    bash "$ROOT/install.sh" >/dev/null 2>&1; then
    echo "installer accepted unknown plugin path" >&2
    exit 1
fi
[[ ! -e "$missing_home/.local/bin/mendeley-loopback-server" ]]

printf '%s\n' "Linux installer path detection passed"
