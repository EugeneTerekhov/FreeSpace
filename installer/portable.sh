#!/usr/bin/env bash
# Builds the portable Linux archive of FreeSpace.
#
# Produces dist/freespace-portable-<version>-linux-x86_64.tar.gz with a single
# top-level `freespace-portable/` directory containing the GUI, the CLI, the
# documentation, CHANGELOG.md, an application-owned `data/` directory and a
# `run.sh` launcher.
#
# The version is read from `[workspace.package].version` in the root Cargo.toml;
# this script contains no version literal.
#
# Usage:
#   bash installer/portable.sh [--skip-build] [--print-version]
#
# The archive is built on Ubuntu 22.04 in CI (glibc baseline). Runtime
# dependencies (GTK 3, WebKitGTK 4.1, libayatana-appindicator3, librsvg2,
# libxdo) stay system libraries and are documented in the archive README.

set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
dist="$root/dist"
ui="$root/apps/freespace-desktop/ui"

skip_build=0
print_version_only=0
for argument in "$@"; do
  case "$argument" in
    --skip-build) skip_build=1 ;;
    --print-version) print_version_only=1 ;;
    --help | -h)
      sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$argument" >&2
      exit 2
      ;;
  esac
done

# Reads `[workspace.package].version`. The table is located from its header so
# dependency versions can never be picked up by mistake.
read_workspace_version() {
  local cargo_toml=$1
  awk '
    /^[[:space:]]*\[/ {
      section = $0
      gsub(/^[[:space:]]*\[[[:space:]]*/, "", section)
      gsub(/[[:space:]]*\][[:space:]]*$/, "", section)
      in_section = (section == "workspace.package")
      next
    }
    in_section && /^[[:space:]]*version[[:space:]]*=/ {
      line = $0
      sub(/^[^"]*"/, "", line)
      sub(/".*$/, "", line)
      print line
      exit
    }
  ' "$cargo_toml"
}

version=$(read_workspace_version "$root/Cargo.toml")
if [ -z "$version" ]; then
  printf 'Unable to read [workspace.package].version from Cargo.toml\n' >&2
  exit 1
fi
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+'; then
  printf 'Unsupported workspace version: %s\n' "$version" >&2
  exit 1
fi

if [ "$print_version_only" -eq 1 ]; then
  printf '%s\n' "$version"
  exit 0
fi

if [ "$skip_build" -eq 0 ]; then
  if [ ! -f "$ui/dist/index.html" ]; then
    printf 'Frontend assets are missing; building them with npm.\n'
    if [ ! -d "$ui/node_modules" ]; then
      ( cd "$ui" && npm ci )
    fi
    ( cd "$ui" && npm run build )
  fi

  # `--no-default-features` disables the optional `context-menu` feature, so
  # the portable build registers nothing in any desktop environment.
  ( cd "$root" && cargo build --release --no-default-features -p freespace-cli -p freespace-desktop )
fi

for binary in "$root/target/release/freespace-gui" "$root/target/release/freespace"; do
  if [ ! -f "$binary" ]; then
    printf 'Missing build artifact %s\n' "$binary" >&2
    exit 1
  fi
done

stage="$dist/freespace-portable"
archive="$dist/freespace-portable-${version}-linux-x86_64.tar.gz"

rm -rf -- "$stage"
mkdir -p -- "$stage/data"

install -m 0755 "$root/target/release/freespace-gui" "$stage/freespace-gui"
install -m 0755 "$root/target/release/freespace" "$stage/freespace"
cp -- "$root/CHANGELOG.md" "$stage/CHANGELOG.md"

# Both language guides are shipped when present; a single legacy guide is used
# as a fallback.
guides=0
for language in ru en; do
  guide="$root/docs/$language/user-guide.md"
  if [ -f "$guide" ]; then
    mkdir -p -- "$stage/docs/$language"
    cp -- "$guide" "$stage/docs/$language/user-guide.md"
    guides=$((guides + 1))
  fi
done
if [ "$guides" -eq 0 ]; then
  if [ -f "$root/docs/user-guide.md" ]; then
    mkdir -p -- "$stage/docs"
    cp -- "$root/docs/user-guide.md" "$stage/docs/user-guide.md"
    guides=1
  else
    printf 'No user guide found (docs/ru, docs/en or docs/user-guide.md)\n' >&2
    exit 1
  fi
fi
printf 'Staged %s user guide(s).\n' "$guides"

cat > "$stage/run.sh" <<'RUN_SH'
#!/bin/sh
# Starts the FreeSpace GUI from this directory. The application resolves its
# data directory relative to the executable, so the working directory does not
# matter.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$here/freespace-gui" "$@"
RUN_SH
chmod 0755 "$stage/run.sh"

cat > "$stage/data/README.txt" <<'DATA_README'
FreeSpace portable data
=======================

Everything the application writes lives in this folder, next to the
executables:

  settings.toml    - user settings
  freespace.db     - journal, snapshots and the hash cache
  webview2/        - the WebView profile

FreeSpace never writes to /etc, /usr, /var, the user's home directory or any
other system location. The XDG trash is only used when you explicitly choose
"move to trash", which is the system recycle bin and is managed by your file
manager. Delete the folder you unpacked the archive into and every trace of the
application is gone. Nothing is installed and nothing is left behind.

Keep this folder writable. If the application is started from a read-only
location, settings live in memory for the session only; there is no fallback to
a system folder, because that would break the portable model.
DATA_README

cat > "$stage/README.txt" <<README
FreeSpace $version - portable Linux x86_64
==========================================

FreeSpace is a disk space analyzer and cleanup tool. This archive is portable:
extract it anywhere, run the binaries, delete the folder when you are done.

Contents
--------

  freespace-gui      - graphical interface
  freespace          - command line interface (every GUI capability has a CLI
                       equivalent with --json output)
  run.sh             - launcher for the GUI
  data/README.txt    - what the application stores and where
  docs/ru, docs/en   - user guide
  CHANGELOG.md       - release history

Getting started
---------------

1. Extract the archive into a folder you can write to, for example
   ~/opt/freespace. A single top-level freespace-portable/ directory is
   created.
2. Verify the CLI:

       cd freespace-portable
       ./freespace --json capabilities

3. Start the GUI, from any working directory:

       ./run.sh

   The application creates its state in data/ next to the executables.

If the executables lost their permission bits during extraction, restore them
with: chmod +x freespace freespace-gui run.sh

Runtime dependencies
--------------------

The archive is not statically linked. The following system libraries must be
installed; they are part of every mainstream desktop distribution and are not
bundled:

  - GTK 3 and WebKitGTK 4.1
      Debian/Ubuntu: libgtk-3-0 libwebkit2gtk-4.1-0
      Fedora:        gtk3 webkit2gtk4.1
  - libayatana-appindicator3
      Debian/Ubuntu: libayatana-appindicator3-1
      Fedora:        libayatana-appindicator-gtk3
  - librsvg2
      Debian/Ubuntu: librsvg2-2
      Fedora:        librsvg2
  - libxdo (xdotool library)
      Debian/Ubuntu: libxdo3
      Fedora:        libxdo
  - GNU C library 2.35 or newer (the archive is built on Ubuntu 22.04)

patchelf is only needed when repackaging or relocating the binaries; it is not
required to run FreeSpace.

If a library is missing, the GUI reports the missing package at startup instead
of failing silently. The CLI has no GUI dependencies and works without them.

Portable model
--------------

Nothing is installed: no packages, no desktop entries, no services, no writes
to system directories. All state stays in data/ next to the executables, so
moving the folder moves the application and deleting it removes every trace.

Deletion uses the XDG trash (your normal file manager's trash) or permanent
deletion, always after a preview and an explicit confirmation. FreeSpace never
downloads anything: the application is fully offline.

Builds are unsigned. Verify SHA256SUMS.txt from the release before running the
archive if you want to check its integrity.
README

rm -f -- "$archive"
tar -czf "$archive" -C "$dist" freespace-portable

printf 'Portable archive written to %s\n' "$archive"
