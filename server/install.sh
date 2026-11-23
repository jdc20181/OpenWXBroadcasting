#!/usr/bin/env bash
# OpenWX Relay - install, update and manage the server on Linux (systemd).
#
# Most stations never need this: the desktop app runs OpenWX Relay on the same PC by itself.
# Use it when several studio PCs should share one relay on your own server.
#
#   sudo bash install.sh                 install (asks a few questions)
#   sudo bash install.sh update          update to the newest release, keep settings
#   sudo bash install.sh setup           change the settings (station, contact, home, token)
#   sudo bash install.sh status          is it running? are the weather feeds answering?
#   sudo bash install.sh logs            follow the log (Ctrl+C to stop)
#   sudo bash install.sh uninstall       remove the service and program (keeps settings)
#   sudo bash install.sh uninstall --purge   also remove settings
#
# Options for install/update:
#   --binary FILE      install this openwx-relay binary instead of downloading one
#   --from-source      build from the source code with Rust (cargo) on this server
#   --version vX.Y.Z   a specific release instead of the newest
#   --yes              accept the defaults without asking (unattended)

set -euo pipefail

REPO="jdc20181/OpenWXBroadcasting"
APP_USER="openwx"
BIN_DIR="/opt/openwx/bin"
BIN="$BIN_DIR/openwx-relay"
CONF_DIR="/etc/openwx"
CONF="$CONF_DIR/relay.toml"
DATA_DIR="/var/lib/openwx"
UNIT="/etc/systemd/system/openwx-relay.service"

COMMAND="install"
BINARY=""
FROM_SOURCE=0
VERSION="latest"
ASSUME_YES=0
PURGE=0

# ---------- output helpers ----------
if [ -t 1 ]; then BOLD=$'\e[1m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; RED=$'\e[31m'; RESET=$'\e[0m'; else BOLD=""; GREEN=""; YELLOW=""; RED=""; RESET=""; fi
say()  { printf '%s\n' "$*"; }
step() { printf '\n%s==> %s%s\n' "$BOLD" "$*" "$RESET"; }
ok()   { printf '%s  OK%s  %s\n' "$GREEN" "$RESET" "$*"; }
warn() { printf '%s  !%s   %s\n' "$YELLOW" "$RESET" "$*"; }
die()  { printf '%sError:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }

ask_yes() { # ask_yes "Question" default(y/n)
  local q="$1" def="${2:-y}" answer
  if [ "$ASSUME_YES" = 1 ] || [ ! -t 0 ]; then [ "$def" = y ]; return; fi
  read -r -p "$q [$( [ "$def" = y ] && echo Y/n || echo y/N )] " answer || true
  answer="${answer:-$def}"
  [[ "$answer" =~ ^[Yy] ]]
}

# ---------- arguments ----------
while [ $# -gt 0 ]; do
  case "$1" in
    install|update|setup|status|logs|uninstall) COMMAND="$1" ;;
    --binary) BINARY="${2:-}"; shift ;;
    --from-source) FROM_SOURCE=1 ;;
    --version) VERSION="${2:-}"; shift ;;
    --yes|-y) ASSUME_YES=1 ;;
    --purge) PURGE=1 ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
  shift
done

need_root() { [ "$(id -u)" -eq 0 ] || die "run this with sudo:  sudo bash $0 $COMMAND"; }
need_systemd() { command -v systemctl >/dev/null 2>&1 || die "this script needs systemd. Use Docker instead: see server/README.md"; }

arch_name() {
  case "$(uname -m)" in
    x86_64|amd64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *) die "no ready-made build for $(uname -m): use --from-source" ;;
  esac
}

fetch() { # fetch URL FILE
  if command -v curl >/dev/null 2>&1; then curl -fsSL --retry 3 -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then wget -q -O "$2" "$1"
  else die "curl or wget is needed"; fi
}

# Puts a verified openwx-relay binary at $1.
get_binary() {
  local out="$1" tmp
  tmp="$(mktemp -d)"
  if [ -n "$BINARY" ]; then
    [ -f "$BINARY" ] || die "no file at $BINARY"
    cp "$BINARY" "$out"
  elif [ "$FROM_SOURCE" = 1 ]; then
    command -v cargo >/dev/null 2>&1 || die "Rust is not installed. Install it from https://rustup.rs (then re-run), or use a release build."
    command -v git >/dev/null 2>&1 || die "git is needed for --from-source"
    step "Building OpenWX Relay from source (a few minutes)"
    local branch_args=()
    [ "$VERSION" != latest ] && branch_args=(--branch "$VERSION")
    git clone --depth 1 ${branch_args[@]+"${branch_args[@]}"} "https://github.com/$REPO.git" "$tmp/src"
    (cd "$tmp/src" && cargo build --release --locked -p openwx-relay)
    cp "$tmp/src/target/release/openwx-relay" "$out"
  else
    local arch base name
    arch="$(arch_name)"
    name="openwx-relay-linux-$arch.tar.gz"
    if [ "$VERSION" = latest ]; then base="https://github.com/$REPO/releases/latest/download"
    else base="https://github.com/$REPO/releases/download/$VERSION"; fi
    step "Downloading $name"
    fetch "$base/$name" "$tmp/$name" || die "no Linux download for this release (or no internet). Build it here with --from-source, or copy one in with --binary."
    fetch "$base/$name.sha256" "$tmp/$name.sha256" || die "the checksum file is missing from the release; refusing to install an unverified download"
    (cd "$tmp" && sha256sum -c "$name.sha256" >/dev/null) || die "checksum mismatch: the download is damaged or was altered"
    ok "checksum verified"
    tar -xzf "$tmp/$name" -C "$tmp"
    cp "$tmp/openwx-relay" "$out"
  fi
  chmod 0755 "$out"
  "$out" --version >/dev/null 2>&1 || die "the binary does not run on this server"
  rm -rf "$tmp"
}

write_unit() {
  cat > "$UNIT" <<UNIT
[Unit]
Description=OpenWX Relay (weather data backend for OpenWX Broadcasting)
Documentation=https://github.com/$REPO
After=network-online.target
Wants=network-online.target

[Service]
User=$APP_USER
Group=$APP_USER
ExecStart=$BIN serve --config $CONF
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
WorkingDirectory=$DATA_DIR
# Hardening: the relay only needs the network and to read its config.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
CapabilityBoundingSet=
ReadOnlyPaths=$CONF_DIR
ReadWritePaths=$DATA_DIR

[Install]
WantedBy=multi-user.target
UNIT
}

port_from_conf() { sed -n 's/^port *= *\([0-9]*\).*/\1/p' "$CONF" 2>/dev/null | head -1; }
bind_from_conf() { sed -n 's/^bind *= *"\(.*\)".*/\1/p' "$CONF" 2>/dev/null | head -1; }

wait_healthy() {
  local port="${1:-8737}" i
  for i in $(seq 1 20); do
    if fetch "http://127.0.0.1:$port/health" /dev/null 2>/dev/null; then return 0; fi
    sleep 1
  done
  return 1
}

https_help() {
  local port; port="$(port_from_conf)"; port="${port:-8737}"
  say ""
  say "${BOLD}HTTPS (needed when studio PCs connect over the internet)${RESET}"
  say "The relay itself speaks plain HTTP. Put it behind HTTPS with Caddy (automatic certificates):"
  say ""
  say "    sudo apt install caddy        # Debian/Ubuntu; see caddyserver.com for others"
  say "    # /etc/caddy/Caddyfile"
  say "    wx.your-domain.org {"
  say "        reverse_proxy 127.0.0.1:$port"
  say "    }"
  say "    sudo systemctl reload caddy"
  say ""
  say "If Caddy runs on this server, keep bind = \"127.0.0.1\" in $CONF so only Caddy can reach it."
}

# ---------- commands ----------
do_install() {
  need_root; need_systemd
  step "Installing OpenWX Relay"
  if [ -x "$BIN" ]; then warn "already installed ($("$BIN" --version)); use 'update' to upgrade. Continuing will reinstall."; ask_yes "Reinstall?" n || exit 0; fi

  id "$APP_USER" >/dev/null 2>&1 || useradd --system --home "$DATA_DIR" --shell /usr/sbin/nologin "$APP_USER"
  install -d -m 0755 "$BIN_DIR"
  install -d -m 0750 -o root -g "$APP_USER" "$CONF_DIR"
  install -d -m 0750 -o "$APP_USER" -g "$APP_USER" "$DATA_DIR"

  get_binary "$BIN.new"
  mv -f "$BIN.new" "$BIN"
  ok "installed $("$BIN" --version)"

  step "Station settings"
  if [ -f "$CONF" ] && ! ask_yes "Settings already exist at $CONF. Change them now?" n; then
    ok "keeping $CONF"
  elif [ "$ASSUME_YES" = 1 ] || [ ! -t 0 ]; then
    [ -f "$CONF" ] || "$BIN" setup --config "$CONF" </dev/null
    warn "unattended install: edit $CONF (contact, home point) or run: sudo bash $0 setup"
  else
    "$BIN" setup --config "$CONF"
  fi
  chown root:"$APP_USER" "$CONF" 2>/dev/null || true
  chmod 0640 "$CONF" 2>/dev/null || true

  write_unit
  systemctl daemon-reload
  systemctl enable --now openwx-relay >/dev/null
  if wait_healthy "$(port_from_conf)"; then ok "running on port $(port_from_conf)"; else warn "the service did not answer yet: sudo bash $0 logs"; fi

  step "Checking the weather feeds"
  sudo -u "$APP_USER" "$BIN" check --config "$CONF" || warn "some feeds did not answer; they may be down for a moment. Re-check with: sudo bash $0 status"

  [ "$(bind_from_conf)" != "127.0.0.1" ] && https_help
  say ""
  ok "Done. In OpenWX Broadcasting: Settings > General > Backend > Hosted server, and paste the address printed above."
}

do_update() {
  need_root; need_systemd
  [ -x "$BIN" ] || die "OpenWX Relay is not installed yet: sudo bash $0 install"
  step "Updating OpenWX Relay (current: $("$BIN" --version))"
  get_binary "$BIN.new"
  if cmp -s "$BIN" "$BIN.new"; then rm -f "$BIN.new"; ok "already up to date"; return; fi
  cp -f "$BIN" "$BIN.previous"
  mv -f "$BIN.new" "$BIN"
  systemctl restart openwx-relay
  if wait_healthy "$(port_from_conf)"; then
    ok "updated to $("$BIN" --version)"
  else
    warn "the new version did not start: rolling back"
    mv -f "$BIN.previous" "$BIN"
    systemctl restart openwx-relay
    die "update rolled back. See: sudo bash $0 logs"
  fi
}

do_setup() {
  need_root
  [ -x "$BIN" ] || die "OpenWX Relay is not installed yet: sudo bash $0 install"
  "$BIN" setup --config "$CONF"
  chown root:"$APP_USER" "$CONF"; chmod 0640 "$CONF"
  systemctl restart openwx-relay && ok "restarted with the new settings"
}

do_status() {
  [ -x "$BIN" ] || die "OpenWX Relay is not installed"
  say "$("$BIN" --version)"
  systemctl --no-pager --lines=0 status openwx-relay || true
  if [ "$(id -u)" -eq 0 ]; then sudo -u "$APP_USER" "$BIN" check --config "$CONF" || true
  else "$BIN" check --config "$CONF" 2>/dev/null || warn "run with sudo to read $CONF"; fi
}

do_uninstall() {
  need_root
  step "Removing OpenWX Relay"
  systemctl disable --now openwx-relay >/dev/null 2>&1 || true
  rm -f "$UNIT"; systemctl daemon-reload || true
  rm -rf "$BIN_DIR"
  if [ "$PURGE" = 1 ]; then rm -rf "$CONF_DIR" "$DATA_DIR"; userdel "$APP_USER" 2>/dev/null || true; ok "removed program, settings and user"
  else ok "removed the program; settings kept in $CONF (use --purge to remove)"; fi
}

case "$COMMAND" in
  install) do_install ;;
  update) do_update ;;
  setup) do_setup ;;
  status) do_status ;;
  logs) need_systemd; journalctl -u openwx-relay -f ;;
  uninstall) do_uninstall ;;
esac
