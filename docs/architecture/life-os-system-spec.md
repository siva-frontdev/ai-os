# LIFE-OS System Specification

> **Status:** Draft (audit-based, v0.1)
> **Scope:** System architecture and reproducible build specification for LIFE-OS.
> **Audit date:** 2026-08-08
> **Source machine:** `ip-172-31-88-92` (development/reference Arch Linux system)

This document is the result of a **read-only audit**. Nothing on the source machine was
modified, installed, removed, or reconfigured. It defines the *target* specification from
which a reproducible `LIFE-OS.iso` can eventually be built.

---

## 1. Executive summary

LIFE-OS is a user-facing operating environment built on top of Arch Linux. Arch/Linux
remains the foundation; a new **LIFE Platform** layer, a **LIFE Session**, and a
**LIFE Open Space** user environment replace the traditional desktop experience.

The current development/reference machine is a **headless cloud VM** (Xen on AWS-class
hardware, legacy BIOS, no GPU, no audio, no desktop, no display manager). It therefore
contains **no traditional desktop stack to remove** — LIFE-OS will be built *greenfield*
on a minimal Arch base, while the existing AI-OS cognitive platform is preserved and
reused wholesale.

The reference machine is a clean, minimal Arch base (282 packages, 34 explicitly installed,
no AUR). It is a strong starting point for a reproducible build. The recommended ISO
strategy is **archiso with a custom `releng`-derived profile** driven by a declarative
package manifest (`life-os-package-manifest.md`), a `platform.toml`-driven first-boot
initializer, and a build pipeline that records package snapshots for traceability.

Key decisions in this document:

| Topic | Decision |
|---|---|
| Linux base | Arch Linux (x86_64, rolling, LTS kernel) — KEEP |
| Desktop environment | None exists today; traditional DE (GNOME/KDE) will NOT be installed on the target |
| Session | New **LIFE Session** (systemd user session + custom compositor approach) |
| User environment | New **LIFE Open Space** (World Store / Documents / AI Core) |
| AI core | Reuse existing AI-OS brain/memory/perception/execution/intelligence crates |
| ISO build | archiso custom profile + package manifest + first-boot initializer |
| Reproducibility | Declarative spec + pinned package snapshot, **not** a disk clone |

---

## 2. Current system audit

### 2.1 Operating system

| Property | Value |
|---|---|
| Distribution | Arch Linux (rolling, `BUILD_ID=rolling`) |
| Architecture | x86_64 |
| Kernel | `6.18.39-1-lts` (LTS, `PREEMPT_DYNAMIC`) |
| systemd | 261.1 |
| Default target | `multi-user.target` (headless, no graphical target) |
| Hostname | `ip-172-31-88-92` |

### 2.2 Hardware (virtualized)

| Component | Value |
|---|---|
| Hypervisor | Xen (`HVM domU`, firmware `4.11.amazon`) |
| CPU | Intel Xeon E5-2686 v4 @ 2.30 GHz, 2 vCPU (1 socket, 2 cores), 45 MiB L3 |
| GPU | Cirrus Logic GD 5446 (virtual VGA, no 3D) |
| RAM | 3.8 GiB (no swap) |
| Storage | `/dev/xvda1`, 100 GiB ext4, single root partition |
| Disk usage | 76.6 GiB used / 17.6 GiB free (82%) |
| Firmware | Legacy BIOS (no EFI) |
| Bootloader | GRUB (`/boot`: `grub`, `vmlinuz-linux-lts`, `initramfs-linux-lts.img`) |

### 2.3 Device/peripheral stack

| Subsystem | Status |
|---|---|
| Audio | **Not installed** (no pipewire, no pulseaudio) |
| Bluetooth | **Not installed** / not running |
| USB | `usbutils` not installed; VM exposes no USB controller |
| Display stack | **None** — no X11, no Wayland, no DRM/GPU drivers |
| Desktop environment | **None** |
| Window manager / compositor | **None** |
| Display manager | **None** |
| Networking | `systemd-networkd` + `systemd-resolved` + `systemd-timesyncd`; DHCP on `enX0` (cloud-init generated config) |
| Session type | `tty` (loginctl shows 4 tty sessions for UID 1000) |

### 2.4 Key findings

1. The machine is a **headless development/reference VM**. There is no graphical stack to
   preserve or remove — LIFE Session is greenfield work.
2. Disk is **82% full** — important for build space planning (archiso needs a working
   directory for the airootfs).
3. **No swap** and only 3.8 GiB RAM — the ISO build and any graphical session will need
   memory budgeting.
4. Cloud-specific packages (`cloud-init`, `aws-cli`, `cloud-guest-utils`) are present.
   These are machine-lifecycle tooling, not part of the intended LIFE-OS target.

---

## 3. Current desktop architecture

There is **no current desktop**. Concretely:

- No `graphical.target` enabled; system boots to `multi-user.target`.
- No `/usr/share/xsessions` or `/usr/share/wayland-sessions` entries.
- No X server, no Wayland compositor, no display manager, no window manager.
- `$DISPLAY`, `$WAYLAND_DISPLAY`, `$XDG_CURRENT_DESKTOP` are unset.
- `/etc/X11` contains only an `xinit` directory; no `xorg.conf`.
- No desktop autostart entries, no user `systemd --user` graphical session targets running.
- User-level `graphical-session.target` and `xdg-desktop-autostart.target` targets exist
  in the unit catalog (systemd ships them) but are inactive.

**Consequence:** the migration path is *"introduce LIFE Session on an empty desktop
canvas"*, not *"replace an existing DE"*. This is the least risky possible starting
point and is fully aligned with the product goal of not building another traditional
desktop.

---

## 4. Linux components we keep

These are the Linux/Arch foundation pieces that MUST remain in LIFE-OS:

| Area | Components | Notes |
|---|---|---|
| Kernel | `linux-lts`, `linux-lts-headers`, `mkinitcpio` | LTS kernel for stability; initramfs generation |
| Boot | `grub`, `dosfstools` (FAT tooling) | BIOS+UEFI boot support on target hardware |
| Base system | `base`, `filesystem`, `pam`, `shadow`, `util-linux` | Core OS |
| Init | `systemd`, `systemd-libs`, `systemd-sysvcompat` | Service/user-session management |
| Networking | `systemd-networkd`, `systemd-resolved`, `systemd-timesyncd`, `iproute2`, `iptables`/`nftables` | Network stack |
| Shell/tools | `bash`, `coreutils`, `findutils`, `grep`, `sed`, `tar`, `zstd`, `xz`, `gzip`, `procps-ng`, `less` | Base utilities |
| Filesystems | `e2fsprogs`, `fuse3`, `btrfs-progs` (target), `util-linux` | Mounting/formatting |
| Security | `openssl`, `openssh`, `ca-certificates`, `gnupg`, `p11-kit`, `audit` | Crypto, SSH, certs, auditing |
| Package mgmt | `pacman`, `pacman-mirrorlist`, `archlinux-keyring`, `reflector` | Lifecycle/updates |
| Runtime libs | `glibc`, `gcc-libs`, `zlib`, `libcap`, `dbus` | System runtime |

These are hardware- and UI-independent. They constitute the **Layer 0 / Layer 1**
(Linux/Arch foundation) of the target architecture and should be treated as immutable
platform plumbing.

---

## 5. Desktop components we eventually remove

**On this reference machine there are none to remove** (no DE packages installed).

For the *target* LIFE-OS image, the rule is: **do not install** traditional desktop
components in the first place. The following class of packages must be excluded from the
LIFE-OS package manifest (they may exist on arbitrary developer machines but not in the
shipped image):

| Class | Examples |
|---|---|
| Desktop environments | `gnome`, `kde-plasma`, `xfce4`, `cinnamon`, `mate`, `lxqt`, `deepin` |
| Desktop shells/panels | `gnome-shell`, `plasma-desktop`, `xfce4-panel`, `lxpanel` |
| Application launchers | `rofi`, `dmenu`, `ulauncher`, `albert` |
| File managers | `nautilus`, `dolphin`, `thunar`, `pcmanfm`, `nemo` |
| Notification systems | `xfce4-notifyd`, `dunst`, `mako`, `notification-daemon` |
| Settings applications | `gnome-control-center`, `systemsettings`, `xfce4-settings` |
| Desktop widgets | `conky`, `plasmoids`, `gnome-extensions` |
| Traditional DM/WM | `gdm`, `sddm`, `lightdm`, `lxdm`, `openbox`, `fluxbox`, `i3`, `sway` (unless reused) |
| Legacy display servers | `xorg-server`, `xorg-apps` (avoid; Wayland-first) |
| Default browsers | `firefox`, `chromium` (LIFE may bundle its own viewer; optional) |
| Office suites | `libreoffice`, `onlyoffice` (LIFE Open Space renders documents itself; optional) |
| Mail/calendar clients | `evolution`, `thunderbird`, `geary` (AI handles mail via Gmail provider) |

Rationale: each of these brings a distinct set of assumptions (a desktop metaphor,
menus, panels, XDG-autostart apps) that conflicts with the LIFE environment's "shared
world" model. LIFE replaces them; shipping both would create two competing surfaces.

---

## 6. Components LIFE will replace

| Traditional concept | LIFE replacement |
|---|---|
| Desktop shell/panel | **LIFE Session** (own session layer, owns window lifecycle) |
| Window manager/compositor | **LIFE Session compositor** (Wayland-based or embedded) |
| File manager + desktop icons | **LIFE Open Space — World Store** (files, knowledge, entities in one graph) |
| Application launcher | **Open Space command/search surface** (CRUD + search over World Store) |
| Notifications | **LIFE notification policy** (Attention → Communication Policy routing) |
| Settings center | **LIFE settings/trust UI** (already exists in companion web UI) |
| Documents/office | **Open Space Documents** (rendering, editing, AI-assisted) |
| Contacts/calendar | **World Store** entity types + Gmail/Calendar/WhatsApp providers |
| Browser | Open Space web/content views (AI-browsable) |
| Traditional login | **LIFE Session** via systemd user session (not a DE display manager) |

The design rule: any capability that is currently a "dead UI with widgets" becomes a
**view over the World Store** in LIFE Open Space. The World Model property graph (AI-OS
ADR-0006) is the single substrate for all of it.

---

## 7. LIFE-OS target architecture

```
                    LIFE-OS

                  LIFE Session
                       │
                  Open Space
                       │
          ┌────────────┼────────────┐
          │            │            │
      World Store   Documents    AI Core
          │            │            │
          └────────────┼────────────┘
                       │
                 LIFE Platform
                       │
              Linux/Arch foundation
                       │
                    Hardware
```

### 7.1 Linux/Arch foundation (kept)

Unchanged Arch base: kernel, systemd, networking, filesystem, security, package manager.
Hardware-specific bits (drivers, firmware) are selected at build time by hardware class
(see §11). No desktop components are installed at this layer.

### 7.2 LIFE Platform

The platform/service layer that hosts all LIFE subsystems:

- **Lifecycle & event bus:** reuse AI-OS `core` (`EventBus`, `Service`, `LifecycleManager`).
- **Runtime:** reuse AI-OS `runtime` + `runtime-manager` + `openclaw-runtime` + `mcp-server`.
- **OS abstraction:** reuse AI-OS OSAL (`crates/osal-*`, `crates/osal-linux`) for
  process/filesystem/network/monitoring access; extend with display/peripheral backends
  for real hardware.
- **Capabilities & policy:** reuse `crates/capabilities`, `crates/capability-policy`
  (risk gating, audit log).
- **Providers:** reuse Gmail, WhatsApp, Telegram, Calendar, GitHub, MCP plugins.
- **Persistence:** World Store backend (SQLite) — new; current JSON-only persistence is
  not sufficient for user-facing CRUD (see §12.5).

### 7.3 AI Core

The existing AI-OS cognitive stack, unchanged and reused wholesale:

- **World Model** (`memory-core` `wm.rs`, `memory-storage` `wm_store.rs`)
- **Memory family** (`memory-*` crates: working, episodic, semantic, knowledge, index, cache, context)
- **Brain family** (`brain/brain-*`: coordinator with `CognitiveLoopService`, planner,
  reasoner, decision, goals, attention, reflection, learning, policy, recovery)
- **Perception family** (`perception/perception-*`)
- **Execution family** (`execution/execution-*`)
- **Intelligence family** (`intelligence/intelligence-*`: LLM coordinator, providers)
- **Companion host** (`brain/brain-coordinator/src/companion_host/`: observation loop,
  cognitive worker, pulse/autonomous tick, communication policy, audit, privacy)

AI Core exposes its capabilities to Open Space over the LIFE Platform API; it does not
own the UI.

### 7.4 World Store

The user's single source of truth for *everything*:

- Entities and relationships (already modelled by the World Model).
- **New:** document store (files, notes, rich text) with metadata in the graph and
  content in managed storage.
- **New:** durable backend (SQLite) replacing the in-memory/JSON world model store.
- **New:** user-facing CRUD, search, filtering, tables, timelines, views, charts.
- All user/AI data — personal, company, career, contacts, projects, goals, missions,
  leads, jobs, knowledge, files, relationships — lives here as typed entities.

### 7.5 Documents

Open Space document subsystem:

- Storage, rendering, editing (markdown/rich text/PDF/images).
- AI-generated visualizations (charts, graphs, maps, diagrams, timelines, whiteboards).
- Direct user manipulation and AI manipulation of the same documents.

### 7.6 Open Space

The primary user-facing environment:

- Canvas/workspace over the World Store (CRUD, search, filtering, tables, timelines,
  whiteboards).
- Document viewer/editor.
- Chat/command surface (existing companion Chat pattern).
- AI-generated visualizations.
- Communication channels (Telegram/WhatsApp/voice) are *interfaces into* the Open Space,
  not the core product.

### 7.7 LIFE Session

The replacement for the traditional desktop login/session:

- A systemd user session (`graphical-session.target`) that starts the LIFE environment.
- Owns windows/compositing (Wayland-based compositor or embedded web runtime).
- Owns login: PAM/logind autologin into the user session (no DE display manager).
- Starts `desktop-companion` + `mcp-server` + Open Space as session-scoped services.

---

## 8. Package specification

Full draft package manifest: **`docs/architecture/life-os-package-manifest.md`**.

Summary by category:

| Category | Contents |
|---|---|
| Required Linux packages | `base`, `linux-lts`, `linux-lts-headers`, `mkinitcpio`, `grub`, `dosfstools`, `systemd`, `systemd-sysvcompat`, `util-linux`, `e2fsprogs`, `pacman`, `archlinux-keyring`, `reflector`, `bash`, `coreutils`, `findutils`, `grep`, `sed`, `gawk`, `tar`, `zstd`, `xz`, `gzip`, `procps-ng`, `less`, `iproute2`, `openssl`, `ca-certificates`, `openssh`, `gnupg`, `git` |
| Graphics packages | Mesa (DRM/EGL), `libdrm`, Vulkan `vulkan-icd-loader` + vendor drivers, `libva`/`vdpau` (vendor variants), `xf86-video-*` only where Wayland fallback needs it |
| Wayland/session | `wayland`, `wayland-protocols`, `libinput`, `libxkbcommon`, a compositor (see §10 options), `xorg-xwayland` (compat, optional) |
| Hardware packages | Vendor: `intel-media-driver`, `libva-intel-driver`; `amd-ucode`/`intel-ucode`; `nvidia-open-dkms` (NVIDIA option); `firmware-linux`, `linux-firmware` |
| Networking | `systemd-networkd`, `systemd-resolved`, `iwd`/`wpa_supplicant`, `dhcpcd` (fallback), `nftables`, `openssh`, `curl` |
| Audio | `pipewire`, `pipewire-pulse`, `wireplumber`, `alsa-utils` (target hardware) |
| Security | `audit`, `nftables`, `openssl`, `ca-certificates`, `pam`, `polkit`, `selinux`-n/a, `apparmor` (optional), `seatbelt`-n/a |
| Runtime deps | `dbus`, `glibc`, `gcc-libs`, `zlib`, `libcap`, `icu`, `libxml2`, `sqlite`, `libsecret`, `hicolor-icon-theme`, `ttf-fonts` (see manifest), Rust toolchain (build-time only) |
| LIFE packages | `ai-os-core`, `ai-os-runtime`, `ai-os-runtime-manager`, `ai-os-mcp-server`, `ai-os-openclaw-runtime`, `brain-*`, `memory-*`, `perception-*`, `execution-*`, `intelligence-*`, `osal-*`, `ai-os-desktop-companion`, `ai-os-desktop-agent`, `life` tool, `life-open-space` (new) |
| MUST NOT exist | GNOME/KDE/XFCE/Cinnamon/MATE, their panels/launchers/file-managers/notifiers/settings-apps, `xorg-server` (unless needed), office suites, traditional DM (`gdm`/`sddm`/`lightdm`) |

---

## 9. Service specification

Target services on a LIFE-OS machine (system scope):

| Service | Role | KEEP/REPLACE |
|---|---|---|
| `systemd-networkd` | Networking | KEEP |
| `systemd-resolved` | DNS | KEEP |
| `systemd-timesyncd` | Time sync | KEEP |
| `sshd` | Remote access (dev) | OPTIONAL |
| `auditd` | Security auditing | KEEP |
| `irqbalance` | IRQ balancing (real hardware) | OPTIONAL |
| `systemd-logind` | Session/login management | KEEP |
| `systemd-udevd` | Device management | KEEP |
| `systemd-homed` | Home areas (optional) | OPTIONAL |
| `dbus-broker` | System bus | KEEP |
| `nftables` | Firewall | KEEP |
| `reflector.timer` | Mirrorlist refresh | OPTIONAL |
| `cloud-init*` | **VM lifecycle — REMOVE from target** | REMOVE |

User scope (new for LIFE):

| Unit | Role |
|---|---|
| `life-session.target` | Wraps `graphical-session.target`; starts LIFE environment |
| `life-open-space.service` | Open Space frontend + API |
| `life-companion.service` | AI-OS `desktop-companion` |
| `life-mcp.service` | `ai-os-mcp-server` (plugins: email, whatsapp, telegram, calendar, github, filesystem) |
| `life-audio.service` | PipeWire/WirePlumber (on target hardware) |
| `life-wm.service` | LIFE compositor |

Timers: `life-backup.timer`, `life-worldmodel-save.timer`, `man-db.timer`, `shadow.timer`
(existing). Sockets: standard systemd + `gpg-agent`/`ssh-agent` remain as-is.

---

## 10. Session/boot architecture

### 10.1 Boot sequence

```
BIOS/UEFI → GRUB → kernel (linux-lts) → initramfs → systemd (multi-user.target)
                                                    │
                                              systemd-logind
                                                    │
                                        autologin user session
                                                    │
                                    graphical-session.target (LIFE Session)
                                    ├── life-wm.service        (compositor)
                                    ├── life-open-space.service
                                    ├── life-companion.service
                                    └── life-mcp.service
```

- No display manager. Login is logind-based (GDM/SDDM-class software is excluded).
- First-boot initializer (`life-firstboot.service`) reads `platform.toml`, generates the
  machine-id, sets hostname, creates the user, runs provider setup (`life setup`), and
  seeds the World Store.

### 10.2 Lifecycle integration

`desktop-companion` already supports autostart via systemd/XDG (`companion_host/autostart.rs`)
and graceful SIGINT/SIGTERM shutdown. LIFE Session reuses this: it starts the companion
as the session's core service and binds Open Space to its web UI + REST/SSE API.

### 10.3 Compositor options (for the implementation phase)

1. **Wayland compositor** (e.g. `sway`, `hyprland`, or a minimal `wlroots`-based
   compositor) — most "own session" approach; recommended direction.
2. **Embedded web runtime** — LIFE Open Space as a fullscreen web app (kiosk) on a
   compositor; fastest to a working prototype and matches the existing companion web UI.
3. **X11 fallback** (only for legacy GPU/support) — not preferred.

Recommendation: start with **option 2** (kiosk-style web runtime over a minimal Wayland
compositor) to reuse the existing companion UI immediately, then evolve the compositor.

---

## 11. Hardware portability

### 11.1 Hardware-independent (LIFE components)

Everything in LIFE Platform, AI Core, World Store, Documents, Open Space, and LIFE Session
logic is hardware-independent. It talks to hardware only through OSAL and vendor-neutral
Linux APIs (libinput, PipeWire, Wayland).

### 11.2 Hardware-specific (Linux config only)

| Domain | Config varies by | Notes |
|---|---|---|
| GPU drivers | Intel / AMD / NVIDIA | Mesa + vendor driver; NVIDIA needs `nvidia-open-dkms` |
| VA-API | Intel / AMD / NVIDIA | `intel-media-driver` / `libva-mesa-driver` / `nvidia-utils` |
| Microcode | Intel / AMD | `intel-ucode` / `amd-ucode` (initramfs) |
| Firmware | Vendor laptops | `linux-firmware` + vendor blobs |
| Audio | SOF / HDA / USB | `sof-firmware`, `alsa-ucm-conf` |
| WiFi/BT | Intel / Realtek / MediaTek / Broadcom | `iwd`, firmware packages |
| Power | Laptop vs desktop | `tlp`/`power-profiles-daemon` (optional) |

**Design rule:** the ISO build selects one of a small set of *hardware profiles*
(desktop-intel, desktop-amd, desktop-nvidia, laptop-intel, laptop-amd, generic) at build
time. LIFE code itself is identical across profiles. This keeps the reproducible spec
small while remaining portable across Intel/AMD/NVIDIA systems, laptops, and desktops.

---

## 12. AI-OS integration

### 12.1 What is reused (no changes)

Reuse the existing AI-OS architecture and code as-is:

| Capability | Where it lives today |
|---|---|
| EventBus / Service / lifecycle | `core` |
| Runtime / scheduler / supervisor / permissions | `runtime` |
| RuntimeManager / capability routing | `runtime/runtime-manager` |
| MCP server + client, plugins (gmail/whatsapp/telegram/calendar/github/filesystem) | `runtime/mcp-server`, `runtime/openclaw-runtime`, `runtime/plugins` |
| OSAL abstraction + Linux impl | `crates/osal-*` |
| Capabilities + policy gate + audit log | `crates/capabilities`, `crates/capability-policy` |
| World Model graph types + store | `crates/memory-core`, `crates/memory-storage` |
| Memory family | `crates/memory-*` |
| Cognitive loop, planner, attention, evolution, recovery, companion host | `brain/brain-coordinator` + `brain/brain-*` |
| Perception pipeline | `perception/perception-*` |
| Execution pipeline | `execution/execution-*` |
| Intelligence coordinator + providers | `intelligence/intelligence-*` |
| Companion UI (REST + SSE + chat) | `brain/brain-coordinator/src/companion_host/ui` |
| OAuth/credential provisioning | `tools/life` |

### 12.2 Runtime locations on the reference machine

- AI-OS repo: `/home/arch/ai-os`
- Companion settings: `~/.config/ai-os-companion/settings.json`
- Companion data: `~/.local/share/ai-os-companion/` (`world_model.json`,
  `telegram_identities.json`, `audit_log.json`)
- Companion UI: `http://127.0.0.1:9876`
- Runtime credentials: `/home/arch/ai-os/runtime/config/.env` (mode 0600) — see §15
- MCP server: `/home/arch/ai-os/target/debug/ai-os-mcp-server`
- Companion binary: `/home/arch/ai-os/target/debug/desktop-companion`
- OpenClaw runtime tree: `/home/arch/openclaw` (separate checkout)

### 12.3 Existing file/document support

- Persistence is JSON-file based (world model, settings, audit, telegram identities).
- `MemoryStore`/`StorageBackend` traits exist but only in-memory/mock backends are
  implemented (SQLite/RocksDB documented as future).
- No document store exists. The companion UI is the only user surface.

### 12.4 What must be built (new)

1. **World Store durable backend** (SQLite) behind the existing `WorldModelStore`/
   `MemoryStore` traits.
2. **Document store** (content + metadata; render/edit APIs).
3. **Open Space frontend** (evolved from the companion web UI).
4. **LIFE Session** (systemd user session + compositor choice).
5. **Provider onboarding for end users** (`life setup` flows extended).
6. **Build pipeline** (archiso profile, manifest, first-boot initializer).

### 12.5 Integration boundary

The companion host currently owns the cognitive loop, observation loop, pulse, and UI
server. LIFE keeps this ownership and **adds** a service layer (World Store API, Document
API, Open Space frontend) that talks to AI Core through the existing decision/notification
interfaces. No cognitive code is rewritten.

---

## 13. ISO/reproducible build strategy

### 13.1 Recommended approach: archiso custom profile

Arch's officially supported, maintained mechanism is **archiso** (`mkarchiso`), used for
the official monthly ISO. It is the most maintainable option and is designed exactly for
"specified image, not clone":

- **releng profile** as the base (hybrid BIOS/UEFI, El Torito + isohybrid).
- **`packages.x86_64`** = the declarative package manifest (`life-os-package-manifest.md`
  becomes the source of truth for this file).
- **`airootfs/`** overlays = base configuration (systemd units, `platform.toml`,
  first-boot initializer, user skeleton, session units).
- **`profiledef.sh`** = ISO identity (label, publisher, file permissions).
- **`-w` working dir on tmpfs** to speed up builds (reference machine has 3.8 GiB RAM;
  prefer a build host or add a `-w` dir on disk with space).
- `mkarchiso` records `pkglist.txt` per build → **package snapshot for traceability**,
  giving reproducibility without a disk clone.

### 13.2 Alternatives evaluated

| Approach | Verdict |
|---|---|
| Disk clone (`dd`/`rsync` copy) | ❌ Rejected — machine-specific, not reproducible |
| Package manifests (plain) | ⚠️ Needed as *input* to archiso; insufficient alone |
| Pacman package groups | ⚠️ Groups are coarsely scoped; manifest is explicit and auditable |
| systemd configuration bundle | ✅ Part of `airootfs` overlay, not a standalone mechanism |
| Custom installation profiles | ✅ This is what archiso profiles are |
| First-boot initialization | ✅ `life-firstboot.service` + `platform.toml` on first run |
| archiso-manager | ⚠️ Useful for CI-style scheduled rebuilds |

### 13.3 Reproducibility additions

- Record **exact package versions** (pacman database + `pkglist.txt`) per build.
- Record the **AI-OS git commit** baked into the image.
- Pin **Cargo.lock** (already in repo) for Rust crates.
- Optionally pin mirrorlist and rebuild from a snapshot mirror for bit-level
  reproducibility.
- `SOURCE_DATE_EPOCH` for deterministic ISO timestamps (supported by mkarchiso).

### 13.4 Build pipeline (future, not built now)

```
life-os-manifest.md ──► packages.x86_64
airootfs overlay ─────► profile/airootfs
platform.toml ────────► first-boot config
AI-OS build (cargo) ──► target/release binaries ──► profile/airootfs/opt/life
mkarchiso ────────────► LIFE-OS.iso + pkglist snapshot
```

---

## 14. Security considerations

- **Default deny:** Open Space capabilities are gated by `capability-policy` (RiskLevel,
  confirmation, audit log). Every Open Space action is authorized and audited.
- **Sessions & permissions:** reuse Runtime role-based permissions
  (Admin/User/ReadOnly) for all user-facing APIs.
- **Network exposure:** Open Space binds to `127.0.0.1` by default (like the companion
  UI today); remote access only via explicit config/SSH tunnel.
- **Secrets:** credentials live in mode-0600 env files / keyring; never in the image or
  logs. `.env` is gitignored. The final ISO ships **no credentials**.
- **Auditing:** `auditd` kept; AI-OS `AuditLog` kept and extended to Open Space actions.
- **Supply chain:** `SigLevel = PackageRequired` (already set), `archlinux-keyring` kept,
  `cargo audit` gate on the Rust workspace.
- **Systemd hardening:** user-scope service units get `ProtectSystem`, `PrivateTmp`,
  `NoNewPrivileges` where compatible with the companion.
- **Legacy surfaces:** because no traditional DE is installed, common desktop attack
  surface (privileged helpers, dbus activation of DE services, X11 keylogging) is absent.

---

## 15. Data/credential separation

### 15.1 Sensitive locations (identified, contents NOT reproduced)

| Location | What it holds | Handling |
|---|---|---|
| `/home/arch/ai-os/runtime/config/.env` | Runtime provider credentials (Gmail/WhatsApp OAuth) | Mode 0600; gitignored; not shipped in ISO |
| `~/.config/ai-os-companion/.encryption_key` | Companion data encryption key | Mode 0400 |
| `~/.config/ai-os-companion/settings.json` | Settings incl. provider tokens | Not shipped; provisioned at first boot |
| `~/.ssh/` | SSH keys | Not shipped |
| `~/.vscode-server`, `~/.copilot`, `~/.gemini`, `~/.opencode` | Dev tooling state/tokens | Dev-machine only; not in image |
| `~/.local/share/ai-os-companion/` | World model, identities, audit log | **User data** — never in image; backed up separately |

### 15.2 Separation rules for LIFE-OS

1. **Image vs data:** the ISO contains the system + application binaries and *zero* user
   data, zero credentials, zero world-model state.
2. **Data locations:** all user/AI data under `~/.local/share/life-os/` (world store,
   documents, backups); config under `~/.config/life-os/`; state under `~/.local/state/`.
3. **Secrets:** provider credentials via `life setup` OAuth flows at first boot, stored in
   a keyring or mode-0600 env file — never baked in.
4. **Backup:** reuse companion `PrivacyManager` backup/restore (AES) covering the World
   Store and Documents.

---

## 16. Migration plan

```
Current Arch development machine  (this VM — audit complete)
        ↓
LIFE development                   (same machine: keep AI-OS dev loop; add build pipeline)
        ↓
LIFE Session prototype             (user session + kiosk web runtime over minimal compositor)
        ↓
Open Space                         (World Store + Documents + UI evolved from companion)
        ↓
System integration                 (companion, MCP, Open Space as session services)
        ↓
Minimal LIFE environment           (dedicated test install with no DE, using the manifest)
        ↓
LIFE-OS build                      (archiso profile + first-boot initializer)
        ↓
LIFE-OS.iso
        ↓
Test machine                       (VM/physical, hardware profile validated)
        ↓
Production machine
```

### 16.1 When is it safe to remove the traditional desktop?

- **On this machine:** there is no desktop to remove — nothing to do.
- **On a target machine with a DE:** the DE becomes safe to remove only when the
  following are all true:
  1. LIFE Session boots reliably to the Open Space for the target user.
  2. Login/autologin, network, audio, and input are fully handled by LIFE Session +
     systemd (no DE helper required).
  3. The traditional DE has been a non-default session for at least one full release
     cycle, with fallback available.
  4. All user data (contacts, documents, calendar) has been migrated into the World
     Store and validated.
  5. The DE packages have been removed from the *manifest* and verified on a clean
     install, not by incremental uninstall on a live box.
- Rule of thumb: **remove in the image, not on the live machine.** The safest transition
  is a fresh `LIFE-OS.iso` install; gradual removal on a running DE system is higher risk.

---

## 17. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| Headless VM ≠ real hardware | High | Build hardware profiles; validate on physical test machines before production |
| No existing desktop to dogfood on | Medium | Prototype LIFE Session on the dev VM with a virtual GPU/Xvfb; iterate |
| Low RAM (3.8 GiB) for builds + GUI | Medium | Build on dedicated builder; keep target image lean |
| JSON-only persistence insufficient for CRUD | High | Add SQLite World Store behind existing traits (no cognitive rewrite) |
| Two engines (BrainOrchestrator vs CognitiveLoopService) disconnected | Medium | Keep current companion loop as the single runtime; do not re-wire during LIFE build |
| LLM provider portability (only OpenAI-compatible today) | Medium | Abstract via existing `IntelligenceCoordinator`; add providers later |
| Wayland compositor maturity for "own session" | Medium | Start with kiosk web runtime; evolve compositor |
| Rolling-release drift breaks reproducibility | Medium | Pin package snapshot + `pkglist.txt`; rebuild from pinned snapshot; pin AI-OS commit |
| Disk-full (82%) blocks build working dir | High | Build with `-w` on a dedicated partition/tmpfs or clean cache first |

---

## 18. Next implementation milestones

1. **M0 — Spec freeze:** accept this document + package manifest (RFC + ADR).
2. **M1 — Build pipeline scaffold:** archiso `life` profile skeleton; manifest →
   `packages.x86_64`; first `LIFE-OS.iso` (base system, no LIFE apps).
3. **M2 — World Store backend:** SQLite behind `WorldModelStore`/`MemoryStore` traits;
   migration of JSON world model.
4. **M3 — LIFE Session prototype:** systemd user session; kiosk web runtime over a minimal
   Wayland compositor; companion + MCP as session services.
5. **M4 — Open Space v1:** World Store CRUD/search UI; documents; chat; AI visualizations
   via existing intelligence coordinator.
6. **M5 — Hardware profiles:** desktop/laptop Intel/AMD/NVIDIA images validated on test
   machines.
7. **M6 — Lifecycle:** first-boot initializer, backups, update path.
8. **M7 — Production:** sign/release process, verified install on production hardware.

---

## Appendix A — Reference machine package snapshot (explicit, 34)

`aws-cli, base, base-devel, clang, cloud-init, cloud-utils, cmake, dosfstools, ex-vi-compat,
fd, git, go, grub, htop, irqbalance, jq, linux-lts, linux-lts-headers, man-db, mkinitcpio,
nano, nodejs, npm, openbsd-netcat, openssh, python, python-pip, reflector, ripgrep, rust,
tmux, tree, unzip, zip`

Total installed: **282** (includes implicit dependencies of the above). AUR packages: **0**.

## Appendix B — Data inventory (paths only, no contents)

- `/home/arch/ai-os` — AI-OS workspace (git: `git@github.com:siva-frontdev/ai-os.git`,
  branch `feature/runtime`)
- `/home/arch/openclaw` — OpenClaw runtime checkout
- `/home/arch/.config/ai-os-companion/` — settings + encryption key
- `/home/arch/.local/share/ai-os-companion/` — world model, identities, audit log
- `/home/arch/.ssh/`, `~/.vscode-server/`, `~/.copilot/`, `~/.gemini/`, `~/.opencode/`,
  `~/.cargo/`, `~/.npm/`, `~/.dotnet/` — dev tooling/credentials (dev-machine only)
- `/etc/ssh/`, `/etc/cloud/`, `/etc/credstore*` — system credentials/config
