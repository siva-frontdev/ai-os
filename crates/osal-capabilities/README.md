# osal-capabilities — Capability-based Authorization

Defines the capability framework used by every OSAL subsystem. A **Capability** is the unit of authorization: every privileged OSAL operation checks a `Capability` before proceeding. This design follows the principle of least privilege — each operation requires exactly the capability it needs, and no more.

## Core Concepts

### Capability
A fine-grained permission token. Examples:
- `FileRead("/etc/passwd")` — permission to read a specific file
- `ProcessKill(Pid(1234))` — permission to kill a specific process
- `Admin` — grants all capabilities (superuser)

### CapabilitySet
A `HashSet<Capability>` wrapper with:
- `grant(cap)` — add a capability
- `revoke(cap)` — remove a capability
- `check(cap) -> bool` — test membership; `Admin` always returns `true`
- `check_path(fn, path) -> bool` — test path-based capabilities where `"*"` matches any path
- `extend(iter)` — batch grant

### CapabilityContext
Binds a subject (identified by a string) to a `CapabilitySet`:
```rust
pub struct CapabilityContext {
    pub subject_id: String,
    pub capabilities: CapabilitySet,
}
```

## Capability Enum Variants

| Group       | Variants                                      |
|-------------|-----------------------------------------------|
| Filesystem  | `FileRead`, `FileWrite`, `FileExecute`, `FileDelete`, `FileWatch`, `FileMetadata` |
| Process     | `ProcessSpawn`, `ProcessKill`, `ProcessSuspend`, `ProcessResume`, `ProcessEnumerate` |
| Terminal    | `TerminalExecute`, `TerminalPTY`, `TerminalSignal` |
| Network     | `NetworkConfigure`, `NetworkSocket`, `NetworkDns`, `NetworkFirewall` |
| Monitoring  | `MonitorCpu`, `MonitorMemory`, `MonitorDisk`, `MonitorNetwork`, `MonitorTemperature`, `MonitorProcesses` |
| Devices     | `DeviceEnumerate`, `DeviceAccess` |
| Clipboard   | `ClipboardRead`, `ClipboardWrite` |
| Display     | `WindowList`, `WindowFocus`, `ScreenCapture` |
| Users       | `UserEnumerate`, `UserSwitch`, `GroupEnumerate` |
| System      | `SystemShutdown`, `SystemReboot`, `SystemSleep`, `SystemHibernate` |
| Admin       | `Admin` |

Path-based variants (`FileRead(path)`, `DeviceAccess(path)`, etc.) accept a concrete path or `"*"` to match any path.
Process-targeting variants (`ProcessKill(pid)`, etc.) accept a concrete `Pid` or use logic at a higher level for wildcard matching.
