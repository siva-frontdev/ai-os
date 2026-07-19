use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use osal_core::Fd;
use serde::{Deserialize, Serialize};

/// Flags describing the state and capabilities of a network interface.
///
/// These are orthogonal to the basic up/down status in
/// [`InterfaceInfo`](osal_core::InterfaceInfo) and provide details such as
/// whether the interface supports broadcast, multicast, or is a loopback
/// device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InterfaceFlags {
    /// Interface is administratively up.
    pub up: bool,
    /// Interface supports broadcasting.
    pub broadcast: bool,
    /// Interface is a loopback device.
    pub loopback: bool,
    /// Interface is a point-to-point link.
    pub point_to_point: bool,
    /// Interface supports multicast.
    pub multicast: bool,
}

/// A detailed network interface descriptor.
///
/// Richer than [`InterfaceInfo`](osal_core::InterfaceInfo) — includes
/// the kernel index, separate IPv4/IPv6 address lists, capability flags,
/// MTU, and link speed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name (e.g. `"eth0"`, `"wlan0"`).
    pub name: String,
    /// Kernel interface index.
    pub index: u32,
    /// MAC address string (e.g. `"00:11:22:33:44:55"`), if available.
    pub mac_address: Option<String>,
    /// Assigned IPv4 addresses.
    pub ipv4_addrs: Vec<IpAddr>,
    /// Assigned IPv6 addresses.
    pub ipv6_addrs: Vec<IpAddr>,
    /// Interface capability flags.
    pub flags: InterfaceFlags,
    /// Maximum transmission unit in bytes.
    pub mtu: u32,
    /// Link speed in bits per second.
    pub speed: u64,
}

/// A connected TCP socket descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    /// File descriptor of the socket.
    pub fd: Fd,
    /// Locally bound address.
    pub local_addr: SocketAddr,
    /// Remote peer address.
    pub peer_addr: SocketAddr,
}

/// The system's DNS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// Ordered list of nameserver IP addresses.
    pub nameservers: Vec<IpAddr>,
    /// DNS search domains appended to single-label queries.
    pub search_domains: Vec<String>,
    /// Additional resolver options (e.g. `"timeout"` -> `"5"`, `"attempts"` -> `"2"`).
    pub options: HashMap<String, String>,
}

/// Result of a ping operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    /// Number of ICMP echo requests transmitted.
    pub transmitted: u32,
    /// Number of ICMP echo replies received.
    pub received: u32,
    /// Packet loss as a percentage (0.0 – 100.0).
    pub loss_percent: f64,
    /// Minimum round-trip time observed.
    pub min_rtt: Duration,
    /// Maximum round-trip time observed.
    pub max_rtt: Duration,
    /// Average round-trip time.
    pub avg_rtt: Duration,
}

/// Internet connectivity status snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityStatus {
    /// Whether the system appears to have internet access.
    pub online: bool,
    /// The name of the interface providing connectivity, if known.
    pub interface: Option<String>,
    /// Observed latency to a known reachable host, in milliseconds.
    pub latency_ms: Option<f64>,
}
