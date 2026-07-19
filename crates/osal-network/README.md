# osal-network — Network Subsystem Trait Definitions

## Architecture

The `NetworkManager` trait provides an operating-system-independent abstraction
over network interfaces, TCP/UDP sockets, DNS resolution, and connectivity
monitoring. All operations that require privilege are guarded by a
`CapabilityContext` parameter.

```
┌──────────────────────────────────────────────────┐
│                  NetworkManager                   │
│  ┌─────────┐ ┌──────────┐ ┌──────┐ ┌──────────┐ │
│  │Interface│ │  Socket  │ │ DNS  │ │Connectiv.│ │
│  │ Mgmt    │ │  Mgmt    │ │Resolv│ │ Monitor  │ │
│  └─────────┘ └──────────┘ └──────┘ └──────────┘ │
│          │            │         │          │      │
│   CapabilityContext checked for every operation   │
└──────────────────────────────────────────────────┘
```

Events flow from the `watch()` channel to allow consumers to react to interface
state changes, connectivity loss, and DNS resolution updates.

---

## Trait: `NetworkManager`

```rust
use async_trait::async_trait;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::mpsc;
use osal_core::{OsalResult, Fd};
use osal_capabilities::CapabilityContext;

#[async_trait]
pub trait NetworkManager: Debug + Send + Sync {
    /// List all network interfaces on the system.
    async fn interfaces(&self, ctx: &CapabilityContext) -> OsalResult<Vec<NetworkInterface>>;

    /// Get a specific network interface by name.
    async fn interface(&self, name: &str, ctx: &CapabilityContext) -> OsalResult<NetworkInterface>;

    /// Create a TCP socket and connect to the given address.
    async fn tcp_connect(&self, addr: &SocketAddr, ctx: &CapabilityContext) -> OsalResult<TcpConnection>;

    /// Create a TCP listener bound to the given address.
    async fn tcp_listen(&self, addr: &SocketAddr, ctx: &CapabilityContext) -> OsalResult<TcpListener>;

    /// Create a UDP socket bound to the given address.
    async fn udp_bind(&self, addr: &SocketAddr, ctx: &CapabilityContext) -> OsalResult<UdpSocket>;

    /// Resolve a hostname to one or more IP addresses via DNS.
    async fn dns_resolve(&self, hostname: &str, ctx: &CapabilityContext) -> OsalResult<Vec<IpAddr>>;

    /// Get the current DNS configuration (nameservers, search domains, options).
    async fn dns_config(&self) -> OsalResult<DnsConfig>;

    /// Ping a host and return latency statistics.
    async fn ping(&self, host: &str, count: u32, ctx: &CapabilityContext) -> OsalResult<PingResult>;

    /// Check overall internet connectivity.
    async fn check_connectivity(&self) -> OsalResult<ConnectivityStatus>;

    /// Subscribe to network events (interface up/down, connectivity, DNS).
    async fn watch(&self) -> mpsc::Receiver<NetworkEvent>;
}
```

---

## Data Types

### `NetworkInterface`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub index: u32,
    pub mac_address: Option<String>,
    pub ipv4_addrs: Vec<IpAddr>,
    pub ipv6_addrs: Vec<IpAddr>,
    pub flags: InterfaceFlags,
    pub mtu: u32,
    pub speed: u64,
}
```

### `InterfaceFlags`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceFlags {
    pub up: bool,
    pub broadcast: bool,
    pub loopback: bool,
    pub point_to_point: bool,
    pub multicast: bool,
}
```

### `TcpConnection`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    pub fd: Fd,
    pub local_addr: SocketAddr,
    pub peer_addr: SocketAddr,
}
```

### `TcpListener` / `UdpSocket`

Re-exported from `tokio::net`:

```rust
pub type TcpListener = tokio::net::TcpListener;
pub type UdpSocket = tokio::net::UdpSocket;
```

### `DnsConfig`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub nameservers: Vec<IpAddr>,
    pub search_domains: Vec<String>,
    pub options: HashMap<String, String>,
}
```

### `PingResult`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub transmitted: u32,
    pub received: u32,
    pub loss_percent: f64,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
    pub avg_rtt: Duration,
}
```

### `ConnectivityStatus`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityStatus {
    pub online: bool,
    pub interface: Option<String>,
    pub latency_ms: Option<f64>,
}
```

---

## Event: `NetworkEvent`

```rust
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    InterfaceUp { name: String, address: IpAddr },
    InterfaceDown { name: String },
    ConnectivityGained,
    ConnectivityLost,
    DnsResolved { hostname: String, addresses: Vec<IpAddr> },
}
```

---

## Error: `NetworkError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("interface not found: {0}")]
    InterfaceNotFound(String),

    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("connection timed out: {0}")]
    ConnectionTimeout(String),

    #[error("DNS resolution failed: {0}")]
    DnsResolveFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("socket error: {0}")]
    SocketError(String),

    #[error("firewall blocked: {0}")]
    FirewallBlocked(String),

    #[error("network unreachable: {0}")]
    NetworkUnreachable(String),

    #[error("I/O error: {0}")]
    IoError(String),
}
```
