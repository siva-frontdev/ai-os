# OSAL Linux — Linux Kernel Bridge

This crate implements all OSAL subsystem traits using Linux-specific APIs (libc, syscalls, procfs, sysfs).
It is the **only** crate in the entire platform allowed to use `unsafe` for FFI with libc.

## Architecture

`LinuxKernelFacade::new()` constructs a `KernelFacade` with every subsystem wired to its Linux backend.

```
LinuxKernelFacade::new() -> KernelFacade
  ├── filesystem: Arc<LinuxFileSystem>
  ├── process:    Arc<LinuxProcessManager>
  ├── terminal:   Arc<LinuxTerminal>
  ├── network:    Arc<LinuxNetworkManager>
  ├── monitoring: Arc<LinuxSystemMonitor>
  ├── devices:    Arc<LinuxDeviceManager>
  ├── users:      Arc<LinuxUserManager>
  └── platform:   Arc<LinuxPlatformInfo>
```

## Subsystem Implementations

### `LinuxFileSystem` — `impl FileSystem`
| Feature | Linux API |
|---|---|
| Async read/write | `tokio::fs` |
| Metadata (stat) | `libc::stat`, `libc::lstat` |
| Permissions | `libc::chmod`, `libc::access` |
| Rename / delete | `libc::rename`, `libc::unlink`, `libc::rmdir` |
| Directory ops | `libc::mkdir`, `libc::opendir` / `readdir` |
| Symlinks | `libc::symlink`, `libc::readlink` |
| Temp files | `libc::mkstemp` |
| Canonicalize | `libc::realpath` |

### `LinuxProcessManager` — `impl ProcessManager`
| Feature | Linux API |
|---|---|
| Spawn | `libc::fork` + `libc::execvp` |
| Managed spawn | `tokio::process::Command` |
| Wait | `libc::waitpid` |
| Kill / signal | `libc::kill` |
| Get PID | `libc::getpid` |
| Enumerate | `/proc/[pid]/stat`, `/proc/[pid]/status` |

### `LinuxTerminal` — `impl Terminal`
| Feature | Linux API |
|---|---|
| PTY open | `libc::posix_openpt` |
| PTY setup | `libc::grantpt`, `libc::unlockpt` |
| PTY name | `libc::ptsname` |
| Resize | `libc::ioctl` with `TIOCSWINSZ` / `TIOCGWINSZ` |
| Read/write | `libc::read`, `libc::write` on PTY fd |

### `LinuxNetworkManager` — `impl NetworkManager`
| Feature | Linux API |
|---|---|
| Interface list | `rtnetlink` (`NETLINK_ROUTE`, `RTM_GETLINK`) |
| Address list | `rtnetlink` (`RTM_GETADDR`) |
| Sockets | `libc::socket`, `libc::bind`, `libc::connect` |
| Listen / accept | `libc::listen`, `libc::accept` |
| DNS resolve | `libc::getaddrinfo` |

### `LinuxSystemMonitor` — `impl SystemMonitor`
| Feature | Linux API |
|---|---|
| CPU usage | `/proc/stat` parsing |
| Memory | `/proc/meminfo` parsing |
| Disk | `libc::statvfs` |
| Network I/O | `/proc/net/dev` parsing |
| Load average | `libc::getloadavg`, `/proc/loadavg` |
| Uptime | `libc::sysinfo`, `/proc/uptime` |
| Process count | `/proc` directory enumeration |

### `LinuxDeviceManager` — `impl DeviceManager`
| Feature | Linux API |
|---|---|
| Device enumeration | `udev` (`udev_enumerate_devices`) |
| Device info | `udev` device properties |
| Open / close | `libc::open`, `libc::close` |
| ioctl | `libc::ioctl` |
| Sysfs | `/sys/class`, `/sys/bus` traversal |

### `LinuxUserManager` — `impl UserManager`
| Feature | Linux API |
|---|---|
| User by UID | `libc::getpwuid_r` |
| Group by GID | `libc::getgrgid_r` |
| User list | `/etc/passwd` line-by-line |
| Group list | `/etc/group` line-by-line |
| Authentication | PAM (`libpam`) via `libc::dlopen` (future) |
| Home / shell | `getpwuid_r` fields |

### `LinuxPlatformInfo` — `impl PlatformInfo`
| Feature | Linux API |
|---|---|
| OS info | `/etc/os-release` parsing |
| Kernel info | `libc::uname` |
| Hardware | `/sys/class/dmi/id/` (product_name, serial, sys_vendor) |
| Boot time | `libc::sysinfo` uptime calculation |
| Hostname | `libc::gethostname`, `libc::sethostname` |
| Environment | `std::env::vars()`, `std::env::set_var()` |
| Directories | XDG basedir spec + `libc::getpwuid_r` |
| CPU count | `libc::sysconf(_SC_NPROCESSORS_ONLN)` |
| Page size | `libc::sysconf(_SC_PAGESIZE)` |
| Host ID | `libc::gethostid` |
