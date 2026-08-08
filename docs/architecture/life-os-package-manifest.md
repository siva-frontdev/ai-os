# LIFE-OS Package Manifest

> **Status:** Draft v0.1 (audit-based)
> **Purpose:** Proposed package categories and membership for the LIFE-OS image.
> This document is the intended **source of truth** for the LIFE-OS build:
> the `packages.x86_64` file of the archiso profile is generated from it.
> **Audit date:** 2026-08-08 · Machine: `ip-172-31-88-92` (headless Arch VM)

Conventions:

- **CORE** = required in every image (hardware-independent).
- **HW-&lt;profile&gt;** = hardware-specific, selected by hardware profile at build time.
- **OPT** = optional, enabled via profile flags (dev/remote/perf).
- **EXCLUDE** = must never appear in the shipped image.
- `(*)` = present on the reference machine today.

---

## 1. Required Linux packages (CORE)

Base OS, boot, filesystem, init, package management.

```
base
base-devel(*)          # build toolchain (builder image; optionally pruned in final)
linux-lts(*)           # LTS kernel (reference choice; alternative: linux)
linux-lts-headers(*)
mkinitcpio(*)
mkinitcpio-busybox
grub(*)
dosfstools(*)
efibootmgr             # UEFI target support (reference VM is BIOS; target may be UEFI)
systemd
systemd-sysvcompat
util-linux
util-linux-libs
e2fsprogs
btrfs-progs            # optional filesystem choice for target data partition
f2fs-tools             # optional
pacman
pacman-mirrorlist(*)
archlinux-keyring(*)
reflector(*)
gawk
grep
sed
findutils
coreutils
diffutils
tar
gzip
xz
zstd
bzip2
bash
less
which
procps-ng
psmisc
nano                 # minimal editor for recovery (alternative: vim)
openssl
ca-certificates
ca-certificates-mozilla
ca-certificates-utils
openssh(*)
git(*)
curl
wget
```

## 2. Required graphics packages (CORE + HW-&lt;GPU&gt;)

Common (CORE):

```
mesa
libdrm
libxkbcommon
vulkan-icd-loader
libva
libvdpau
xf86-video-vesa        # fallback only
```

Intel (HW-intel):

```
mesa
libva-intel-driver
intel-media-driver
vulkan-intel
intel-ucode
```

AMD (HW-amd):

```
mesa
libva-mesa-driver
vulkan-radeon
amd-ucode
```

NVIDIA (HW-nvidia):

```
nvidia-open-dkms        # or nvidia-dkms / nvidia depending on generation
nvidia-utils
libva-nvidia-driver     # via extra repo; optional
```

## 3. Wayland / session packages (CORE)

```
wayland
wayland-protocols
libinput
libxcrypt
xorg-xwayland          # OPT: X11 compat inside Wayland
```

Compositor choice is a build option (one of):

- `hyprland` — featureful Wayland compositor (recommended direction)
- `sway` — stable i3-compatible Wayland compositor
- `wlroots` — minimal compositor building blocks (if building a LIFE-native compositor)

## 4. Required hardware packages (HW-&lt;profile&gt;)

```
linux-firmware
intel-ucode        # HW-intel
amd-ucode          # HW-amd
sof-firmware       # audio DSP, laptops
alsa-ucm-conf      # audio UCM
firmware-b43legacy # legacy WiFi (rare)
```

## 5. Required networking packages (CORE)

```
systemd-networkd     # shipped with systemd
systemd-resolved
systemd-timesyncd
iwd                  # WiFi (alternative: wpa_supplicant)
dhcpcd               # fallback DHCP client
nftables             # firewall (also iptables-nft compatibility)
iproute2
iputils
bind-tools           # dig/host (OPT)
openresolv           # OPT
```

## 6. Required audio packages (CORE on desktop profiles)

```
pipewire
pipewire-pulse
wireplumber
alsa-utils
alsa-lib
libpulse             # compat for legacy apps (OPT)
```

## 7. Required security packages (CORE)

```
audit(*)
nftables
openssl
ca-certificates
pam
pambase
polkit               # privilege escalation policy (target)
shadow
apparmor             # OPT: MAC profile
sudo
cryptsetup           # disk encryption (target data partition)
libsecret
pinentry
gnupg(*)
```

## 8. Required LIFE packages (CORE — from the AI-OS workspace)

These are the existing AI-OS crates (reused unchanged) plus the new LIFE components.

Existing AI-OS binaries/services:

```
ai-os-desktop-companion     # companion host + web UI (brain-coordinator)
ai-os-desktop-agent         # CLI agent/execution engine
ai-os-mcp-server            # MCP server binary (runtime/plugins)
ai-os-runtime-api
ai-os-runtime-manager
ai-os-openclaw-runtime
ai-os-runtime-bootstrap
life                        # provider setup CLI (tools/life)
```

Existing AI-OS library crates (shipped with the binaries above):

```
ai-os-core, ai-os-runtime
ai-os-osal-core, ai-os-osal-linux, ai-os-osal-capabilities,
ai-os-osal-filesystem, ai-os-osal-process, ai-os-osal-terminal,
ai-os-osal-network, ai-os-osal-monitoring, ai-os-osal-devices,
ai-os-osal-users, ai-os-osal-platform
ai-os-capabilities, ai-os-capability-policy
ai-os-memory-core, ai-os-memory-storage, ai-os-memory-index,
ai-os-memory-working, ai-os-memory-context, ai-os-memory-cache,
ai-os-memory-episodic, ai-os-memory-semantic, ai-os-memory-knowledge
ai-os-brain-core, ai-os-brain-model, ai-os-brain-policy, ai-os-brain-goals,
ai-os-brain-planner, ai-os-brain-reasoner, ai-os-brain-decision,
ai-os-brain-reflection, ai-os-brain-learning, ai-os-brain-workflow,
ai-os-brain-coordinator
ai-os-perception-core, ai-os-perception-observer, ai-os-perception-normalizer,
ai-os-perception-detector, ai-os-perception-entities, ai-os-perception-context,
ai-os-perception-anomaly, ai-os-perception-fusion, ai-os-perception-coordinator
ai-os-execution-core, ai-os-execution-registry, ai-os-execution-planner,
ai-os-execution-dispatcher, ai-os-execution-runner, ai-os-execution-sandbox,
ai-os-execution-monitor, ai-os-execution-results, ai-os-execution-recovery,
ai-os-execution-discovery, ai-os-execution-adapter, ai-os-execution-cache,
ai-os-execution-resolution, ai-os-execution-coordinator
ai-os-intelligence-core, ai-os-intelligence-providers, ai-os-intelligence-models,
ai-os-intelligence-router, ai-os-intelligence-prompts, ai-os-intelligence-context,
ai-os-intelligence-embeddings, ai-os-intelligence-cache, ai-os-intelligence-streaming,
ai-os-intelligence-tools, ai-os-intelligence-cost, ai-os-intelligence-safety,
ai-os-intelligence-telemetry, ai-os-intelligence-coordinator
```

New LIFE components (to be built; see `life-os-system-spec.md` §12.4):

```
life-open-space           # Open Space frontend + API (new)
life-session              # session/compositor integration (new)
life-worldstore           # durable World Store backend, SQLite (new)
life-documents            # document store (new)
life-firstboot            # first-boot initializer (new)
```

## 9. Required runtime dependencies (CORE)

```
dbus
dbus-broker(*)
glibc
gcc-libs
zlib
zlib-ng
libcap
icu
libxml2
libevent
libpng
libjpeg-turbo
libwebp
sqlite
libsecret(*)
hicolor-icon-theme(*)
fontconfig
freetype2
ttf-dejavu               # default UI font
ttf-nerd-fonts-symbols   # OPT: UI icons
noto-fonts               # OPT: broader glyph coverage
xkeyboard-config
shared-mime-info
desktop-file-utils
xdg-utils
```

Build-time only (builder image, not shipped):

```
base-devel, gcc, clang, cmake, go, rust, nodejs, npm, python, python-pip,
lld, llvm-libs, linux-lts-headers, jq, fd, ripgrep, tmux, htop, tree, unzip, zip
```

## 10. Packages that MUST NOT exist in the final LIFE-OS image (EXCLUDE)

Traditional desktop environment and desktop-metaphor components:

```
gnome, gnome-shell, gnome-session, gnome-control-center, gnome-terminal,
nautilus, gnome-tweaks, gnome-system-monitor, gnome-software, gnome-maps,
gnome-photos, gnome-calendar, evolution, eog, totem, gedit, gnome-shell-extensions
kde-plasma, plasma-desktop, plasma-workspace, systemsettings, konsole,
dolphin, ark, okular, kate, ksysguard, kmail, korganizer, kontact,
plasma-nm, plasma-pa, spectacle, gwenview, elisa, filelight, partitionmanager
xfce4, xfce4-session, xfce4-panel, xfce4-settings, xfce4-terminal,
thunar, xfce4-notifyd, xfce4-power-manager, xfce4-appfinder, xfwm4
cinnamon, cinnamon-session, nemo, mate, mate-desktop, lxqt, lxde, deepin
gdm, sddm, lightdm, lxdm, slim
openbox, fluxbox, i3, i3status, swaylock, i3lock (unless reused)
xorg-server, xorg-apps, xorg-xinit (unless explicitly needed for fallback)
rofi, dmenu, ulauncher, albert
conky, cairo-dock, plank
evolution-data-server (unless needed as provider backend)
libreoffice, onlyoffice, calligra, abiword, gnumeric (LIFE renders its own docs)
firefox, chromium, google-chrome (OPT only; not part of the core image)
```

Rule: the image must boot to a LIFE Session and the Open Space surface; any package that
pulls in a desktop-environment dependency chain is excluded.

## 11. Reference-machine classification (KEEP / REMOVE / REPLACE / OPTIONAL / UNKNOWN)

Classification of the 282 packages currently on the reference machine, by role. This is
audit-only; nothing is uninstalled.

### 11.1 KEEP (foundation, reuse in LIFE-OS)

`base, base-devel (build), bash, binutils, bzip2, ca-certificates*, coreutils, cpio,
cryptsetup, curl, dbus*, device-mapper, diffutils, e2fsprogs, expat, file, filesystem,
findutils, gawk, gcc, gcc-libs, gettext, glibc, gmp, gnupg, gpgme, grep, gzip, htop (opt),
icu, iproute2, iptables, iputils, jansson, jq, kmod, libarchive, libcap, libffi, libgcrypt,
libgpg-error, libidn2, libmnl, libnetfilter_conntrack, libnfnetlink, libnftnl, libpcap,
libseccomp, libtasn1, libtirpc, libxml2, lz4, make, man-db, mkinitcpio*, mpfr, ncurses,
nftables, openssh*, openssl, p11-kit, pacman*, pam, pambase, pciutils, pcre2, popt,
procps-ng, psmisc, readline, reflector*, sed, shadow, sqlite, sudo, systemd*, tar, texinfo,
tzdata, util-linux*, which, xz, zlib, zstd, archlinux-keyring, linux-lts*, linux-lts-headers,
grub*, dosfstools, pacman-mirrorlist`

### 11.2 REMOVE (from the *target image*, not the dev machine)

Cloud/VM lifecycle tooling not needed on a physical LIFE-OS install:

```
aws-cli, cloud-init, cloud-utils, cloud-guest-utils, cloud-image-utils, qemu-img,
cdrtools (unless ISO tooling needed on-target), dhclient (replaced by systemd-networkd)
```

Why safe: the target image is installed on physical hardware; these packages only serve
cloud VM provisioning. The dev machine keeps them (it is a cloud VM).

### 11.3 REPLACE

```
reflector → pinned snapshot mirror / build-time only   (image has static mirrorlist)
dhclient → systemd-networkd                            (already default on reference)
nano/vim → kept as recovery editors; no replacement needed
```

### 11.4 OPTIONAL (keep on dev, optional on target)

```
git, htop, jq, fd, ripgrep, tmux, tree, unzip, zip, ex-vi-compat, openbsd-netcat,
vim, wget, go, nodejs, npm, rust, clang, cmake, python, python-pip, gdb, strace-less,
irqbalance (target: OPT for multicore), audit (KEEP)
```

### 11.5 UNKNOWN / transient

Packages that exist only as transitive build/test dependencies on the dev machine and
should not appear in the image: `ada, autoconf, automake, bison, boost-libs, c-ares,
cdrtools (see above), cppdap, debugedit, elfutils, flex, gc, gdb-common, gnulib-l10n,
gpm, guile, hwdata, jemalloc, json-c, jsoncpp, krb5, leancrypto, libaio, libasan, libassuan,
libatomic, libbsd, libcap-ng, libedit, libelf, libevent, libgfortran, libgit2, libgomp,
libhwasan, libisl, libksba, libldap, liblsan, libmd, libmpc, libnghttp2, libnghttp3,
libngtcp2, libnl, libnsl, libobjc, libpipeline, libpsl, libquadmath, libsasl, libssh2,
libstdc++, libsysprof-capture, libtool, libtsan, libubsan, libunistring, liburing, libusb,
libutempter, libuv, libverto, libxcrypt, libyaml, linux-api-headers, llhttp, llvm-libs,
lmdb, m4, mpdecimal, netpbm, node-gyp, nodejs-nopt, npth, numactl, oniguruma, pahole,
patch, perl*, pinentry, python-* (aws/botocore stack), rhash, run-parts, semver, simdjson,
source-highlight, tpm2-tss, xxhash, zlib-ng`

These are correctly resolved by pacman at install time on the target; the manifest lists
the *top-level* intent and lets dependency resolution fill in the rest. They are
categorized UNKNOWN because their presence on this dev box is incidental, not deliberate.

## 12. Manifest → build mapping

The archiso profile (`profiledef.sh` + `packages.x86_64`) is generated from this document:

- `§1–§9` (CORE) → base `packages.x86_64` contents.
- `§2, §4, §6` (HW-*) → selected by `HW_PROFILE` env at build time.
- `§10` (EXCLUDE) → enforced by a post-build audit (`pacman -Q` diff against manifest) in
  CI, not by attempting to "remove" after install.
- `§11.2` (REMOVE) → simply absent from `packages.x86_64`.

This keeps the build **declarative** (specify what to include) rather than **reactive**
(install everything, then delete).
