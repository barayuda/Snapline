#!/usr/bin/env bash
# The OrbStack Ubuntu machine for building and trying Snapline for Linux on a Mac.
#   linux/dev-vm.sh setup       create it (once), install the build packages and Rust
#   linux/dev-vm.sh desktop     add an Xfce desktop to open with Screen Sharing
#   linux/dev-vm.sh flatpak     add flatpak-builder and the Flathub remote
#   linux/dev-vm.sh run CMD...  run CMD in windows-linux/ inside the machine
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd) # windows-linux/, the same path inside the machine
vm=snapline-dev
in_vm() { orb -m "$vm" bash -lc "$1"; }
case "${1:-}" in
setup)
  orb list | grep -q "^$vm " || orb create ubuntu:noble "$vm"
  in_vm "sudo apt-get update && grep -v '^\$' '$here/linux/apt-packages.txt' | xargs sudo apt-get install -y --no-install-recommends"
  in_vm 'command -v cargo >/dev/null || curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
  ;;
desktop)
  in_vm 'sudo apt-get install -y xfce4 xfce4-terminal tigervnc-standalone-server gnome-keyring xdg-utils' # OrbStack's own xdg-open opens files on the Mac; start apps with PATH=/usr/bin:$PATH
  echo "1. Set a VNC password:  orb -m $vm vncpasswd"
  echo "2. Start the desktop:   orb -m $vm vncserver :1 -localhost no -xstartup startxfce4"
  echo "3. In Finder: Go → Connect to Server → vnc://$vm.orb.local:5901"
  ;;
flatpak)
  in_vm 'sudo apt-get install -y flatpak flatpak-builder && flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo'
  ;;
run)
  shift
  in_vm "cd '$here' && CARGO_TARGET_DIR=\$HOME/snapline-target $(printf '%q ' "$@")"
  ;;
*)
  sed -n '2,6p' "$0"
  exit 2
  ;;
esac
