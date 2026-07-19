use criterion::{criterion_group, criterion_main, Criterion};

use osal_capabilities::CapabilityContext;
use osal_network::{
    ConnectivityStatus, DefaultNetworkManager, DnsConfig, InterfaceFlags,
    NetworkInterface, PingResult, TcpConnection,
};

fn context() -> CapabilityContext {
    CapabilityContext::new("bench".into(), "bench-session".into(), vec![])
}

fn bench_default_interfaces(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultNetworkManager;
    let ctx = context();

    c.bench_function("DefaultNetworkManager::interfaces", |b| {
        b.to_async(&rt).iter(|| mgr.interfaces(&ctx))
    });
}

fn bench_default_configure(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultNetworkManager;
    let ctx = context();
    let config = osal_core::NetworkConfig {
        dhcp: true,
        address: None,
        netmask: None,
        gateway: None,
        dns_servers: vec![],
    };

    c.bench_function("DefaultNetworkManager::configure", |b| {
        b.to_async(&rt)
            .iter(|| mgr.configure(&ctx, "eth0", config.clone()))
    });
}

fn bench_default_dns_lookup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultNetworkManager;
    let ctx = context();

    c.bench_function("DefaultNetworkManager::dns_lookup", |b| {
        b.to_async(&rt)
            .iter(|| mgr.dns_lookup(&ctx, "example.com"))
    });
}

fn bench_construct_tcp_connection(c: &mut Criterion) {
    c.bench_function("TcpConnection::new", |b| {
        b.iter(|| TcpConnection {
            fd: osal_core::Fd(3),
            local_addr: "127.0.0.1:8080".parse().unwrap(),
            peer_addr: "10.0.0.1:443".parse().unwrap(),
        })
    });
}

fn bench_construct_dns_config(c: &mut Criterion) {
    c.bench_function("DnsConfig::new", |b| {
        b.iter(|| DnsConfig {
            nameservers: vec!["8.8.8.8".parse().unwrap()],
            search_domains: vec!["example.com".into()],
            options: {
                let mut m = std::collections::HashMap::new();
                m.insert("timeout".into(), "5".into());
                m
            },
        })
    });
}

fn bench_construct_ping_result(c: &mut Criterion) {
    use std::time::Duration;
    c.bench_function("PingResult::new", |b| {
        b.iter(|| PingResult {
            transmitted: 5,
            received: 4,
            loss_percent: 20.0,
            min_rtt: Duration::from_millis(10),
            max_rtt: Duration::from_millis(30),
            avg_rtt: Duration::from_millis(20),
        })
    });
}

fn bench_construct_connectivity_status(c: &mut Criterion) {
    c.bench_function("ConnectivityStatus::new", |b| {
        b.iter(|| ConnectivityStatus {
            online: true,
            interface: Some("eth0".into()),
            latency_ms: Some(12.5),
        })
    });
}

fn bench_construct_network_interface(c: &mut Criterion) {
    c.bench_function("NetworkInterface::new", |b| {
        b.iter(|| NetworkInterface {
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
        })
    });
}

fn bench_serde_tcp_connection(c: &mut Criterion) {
    let conn = TcpConnection {
        fd: osal_core::Fd(3),
        local_addr: "127.0.0.1:8080".parse().unwrap(),
        peer_addr: "10.0.0.1:443".parse().unwrap(),
    };
    c.bench_function("TcpConnection::serialize_json", |b| {
        b.iter(|| serde_json::to_string(&conn))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets =
        bench_default_interfaces,
        bench_default_configure,
        bench_default_dns_lookup,
        bench_construct_tcp_connection,
        bench_construct_dns_config,
        bench_construct_ping_result,
        bench_construct_connectivity_status,
        bench_construct_network_interface,
        bench_serde_tcp_connection,
}

criterion_main!(benches);
