#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL network subsystem — re-exports the core [`NetworkManager`] trait and
//! provides network-specific types and a no-op default implementation.
//!
//! # Re-exports
//!
//! The following types are re-exported from `osal-core`:
//!
//! * [`NetworkManager`] — OS-agnostic network abstraction trait
//! * [`NetworkError`]   — network operation errors
//! * [`InterfaceInfo`]  — network interface descriptor
//! * [`NetworkConfig`]  — interface configuration
//! * [`NetworkIO`]      — I/O statistics
//!
//! # Network-specific types
//!
//! These types are unique to this crate and not part of the core abstraction:
//!
//! * [`NetworkInterface`] — rich interface descriptor (index, flags, MTU, speed)
//! * [`InterfaceFlags`]   — interface capability flags
//! * [`TcpConnection`]    — connected TCP socket handle
//! * [`DnsConfig`]        — system DNS configuration
//! * [`PingResult`]       — ICMP ping statistics
//! * [`ConnectivityStatus`] — internet connectivity state
//!
//! [`TcpListener`] and [`UdpSocket`] are re-exported from `tokio::net`.

mod types;

pub use types::{
    ConnectivityStatus, DnsConfig, InterfaceFlags, NetworkInterface, PingResult, TcpConnection,
};

pub use tokio::net::{TcpListener, UdpSocket};

pub use osal_core::{InterfaceInfo, NetworkConfig, NetworkError, NetworkIO, NetworkManager};

use std::net::IpAddr;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::OsalEvent;
use tokio::sync::mpsc::{self, Receiver};

/// A no-op implementation of [`NetworkManager`] that returns sensible defaults
/// or errors for every operation.
///
/// Useful as a placeholder when the networking subsystem is not available or
/// has not been wired up yet. All mutating and lookup operations return an
/// error; [`interfaces`](NetworkManager::interfaces) returns an empty vec.
#[derive(Debug)]
pub struct DefaultNetworkManager;

#[async_trait]
impl NetworkManager for DefaultNetworkManager {
    async fn interfaces(
        &self,
        _ctx: &CapabilityContext,
    ) -> Result<Vec<InterfaceInfo>, NetworkError> {
        Ok(Vec::new())
    }

    async fn configure(
        &self,
        _ctx: &CapabilityContext,
        _interface: &str,
        _config: NetworkConfig,
    ) -> Result<(), NetworkError> {
        Err(NetworkError::ConfigurationFailed("not available".into()))
    }

    async fn dns_lookup(
        &self,
        _ctx: &CapabilityContext,
        _host: &str,
    ) -> Result<IpAddr, NetworkError> {
        Err(NetworkError::DnsFailed("not available".into()))
    }

    async fn set_hostname(
        &self,
        _ctx: &CapabilityContext,
        _hostname: &str,
    ) -> Result<(), NetworkError> {
        Err(NetworkError::ConfigurationFailed("not available".into()))
    }

    fn events(&self) -> Receiver<OsalEvent> {
        let (_tx, rx) = mpsc::channel(1);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osal_capabilities::CapabilityContext;

    fn test_context() -> CapabilityContext {
        CapabilityContext::new("test-app")
    }

    #[tokio::test]
    async fn default_interfaces_returns_empty() {
        let mgr = DefaultNetworkManager;
        let ctx = test_context();
        let ifaces = mgr.interfaces(&ctx).await.unwrap();
        assert!(ifaces.is_empty());
    }

    #[tokio::test]
    async fn default_configure_returns_error() {
        let mgr = DefaultNetworkManager;
        let ctx = test_context();
        let config = NetworkConfig {
            dhcp: true,
            address: None,
            netmask: None,
            gateway: None,
            dns_servers: vec![],
        };
        let err = mgr.configure(&ctx, "eth0", config).await.unwrap_err();
        assert!(matches!(err, NetworkError::ConfigurationFailed(_)));
    }

    #[tokio::test]
    async fn default_dns_lookup_returns_error() {
        let mgr = DefaultNetworkManager;
        let ctx = test_context();
        let err = mgr.dns_lookup(&ctx, "example.com").await.unwrap_err();
        assert!(matches!(err, NetworkError::DnsFailed(_)));
    }

    #[tokio::test]
    async fn default_set_hostname_returns_error() {
        let mgr = DefaultNetworkManager;
        let ctx = test_context();
        let err = mgr.set_hostname(&ctx, "myhost").await.unwrap_err();
        assert!(matches!(err, NetworkError::ConfigurationFailed(_)));
    }

    #[tokio::test]
    async fn default_events_channel_closed_immediately() {
        let mgr = DefaultNetworkManager;
        let mut rx = mgr.events();
        // The channel has no sender, so recv should return None immediately.
        assert!(rx.recv().await.is_none());
    }

    #[test]
    fn default_network_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultNetworkManager>();
    }

    #[test]
    fn tcp_connection_construct() {
        let conn = TcpConnection {
            fd: osal_core::Fd(3),
            local_addr: "127.0.0.1:8080".parse().unwrap(),
            peer_addr: "10.0.0.1:443".parse().unwrap(),
        };
        assert_eq!(conn.fd.0, 3);
        assert_eq!(conn.local_addr.port(), 8080);
        assert_eq!(conn.peer_addr.port(), 443);
    }

    #[test]
    fn dns_config_construct() {
        use std::collections::HashMap;
        let mut options = HashMap::new();
        options.insert("timeout".into(), "5".into());
        let cfg = DnsConfig {
            nameservers: vec!["8.8.8.8".parse().unwrap()],
            search_domains: vec!["example.com".into()],
            options,
        };
        assert_eq!(cfg.nameservers.len(), 1);
        assert_eq!(cfg.search_domains[0], "example.com");
        assert_eq!(cfg.options.get("timeout").unwrap(), "5");
    }

    #[test]
    fn ping_result_construct() {
        use std::time::Duration;
        let res = PingResult {
            transmitted: 5,
            received: 4,
            loss_percent: 20.0,
            min_rtt: Duration::from_millis(10),
            max_rtt: Duration::from_millis(30),
            avg_rtt: Duration::from_millis(20),
        };
        assert_eq!(res.transmitted, 5);
        assert_eq!(res.received, 4);
        assert!((res.loss_percent - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn connectivity_status_construct() {
        let status = ConnectivityStatus {
            online: true,
            interface: Some("eth0".into()),
            latency_ms: Some(12.5),
        };
        assert!(status.online);
        assert_eq!(status.interface.unwrap(), "eth0");
    }

    #[test]
    fn interface_flags_construct() {
        let flags = InterfaceFlags {
            up: true,
            broadcast: true,
            loopback: false,
            point_to_point: false,
            multicast: true,
        };
        assert!(flags.up);
        assert!(flags.broadcast);
        assert!(flags.multicast);
        assert!(!flags.loopback);
    }

    #[test]
    fn network_interface_construct() {
        let iface = NetworkInterface {
            name: "eth0".into(),
            index: 1,
            mac_address: Some("00:11:22:33:44:55".into()),
            ipv4_addrs: vec!["192.168.1.1".parse().unwrap()],
            ipv6_addrs: vec![],
            flags: InterfaceFlags {
                up: true,
                broadcast: true,
                loopback: false,
                point_to_point: false,
                multicast: true,
            },
            mtu: 1500,
            speed: 1_000_000_000,
        };
        assert_eq!(iface.name, "eth0");
        assert_eq!(iface.index, 1);
        assert!(!iface.flags.loopback);
        assert_eq!(iface.mtu, 1500);
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TcpConnection>();
        assert_send_sync::<DnsConfig>();
        assert_send_sync::<PingResult>();
        assert_send_sync::<ConnectivityStatus>();
        assert_send_sync::<InterfaceFlags>();
        assert_send_sync::<NetworkInterface>();
    }
}
