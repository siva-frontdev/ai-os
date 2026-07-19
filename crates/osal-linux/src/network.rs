use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::mpsc::{self, Receiver};

use osal_core::{
    InterfaceInfo, NetworkConfig, NetworkError, NetworkManager, OsalEvent,
};

pub struct LinuxNetworkManager;

impl LinuxNetworkManager {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxNetworkManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxNetworkManager").finish()
    }
}

#[async_trait]
impl NetworkManager for LinuxNetworkManager {
    async fn interfaces(&self, _ctx: &CapabilityContext) -> Result<Vec<InterfaceInfo>, NetworkError> {
        let mut entries = fs::read_dir("/sys/class/net")
            .await
            .map_err(|e| NetworkError::Io(format!("cannot read /sys/class/net: {e}")))?;

        let mut ip_map: HashMap<String, Vec<IpAddr>> = HashMap::new();

        if let Ok(output) = Command::new("ip").args(["-o", "addr", "show"]).output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let mut parts = line.splitn(4, ' ');
                    let _ifindex = parts.next();
                    let name = parts.next().unwrap_or("").trim_end_matches(':');
                    let _family = parts.next().unwrap_or("");
                    let rest = parts.next().unwrap_or("");

                    let addr_str = rest.split_whitespace().next().unwrap_or("");
                    if let Some(ip_str) = addr_str.split('/').next() {
                        if let Ok(ip) = ip_str.parse::<IpAddr>() {
                            ip_map.entry(name.to_string()).or_default().push(ip);
                        }
                    }
                }
            }
        }

        let mut interfaces = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| NetworkError::Io(format!("readdir error: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = PathBuf::from("/sys/class/net").join(&name);

            let mac_address = fs::read_to_string(base.join("address"))
                .await
                .ok()
                .map(|s| s.trim().to_string());

            let is_up = fs::read_to_string(base.join("operstate"))
                .await
                .ok()
                .map_or(false, |s| s.trim() == "up");

            let addresses = ip_map.remove(&name).unwrap_or_default();

            interfaces.push(InterfaceInfo {
                name,
                addresses,
                mac_address,
                is_up,
            });
        }

        Ok(interfaces)
    }

    async fn configure(
        &self,
        _ctx: &CapabilityContext,
        interface: &str,
        config: NetworkConfig,
    ) -> Result<(), NetworkError> {
        let iface_path = PathBuf::from("/sys/class/net").join(interface);
        if fs::metadata(&iface_path).await.is_err() {
            return Err(NetworkError::InterfaceNotFound(interface.to_string()));
        }

        if let Some(ref addr) = config.address {
            let prefix = match addr {
                IpAddr::V4(_) => "24",
                IpAddr::V6(_) => "64",
            };
            let addr_str = format!("{addr}/{prefix}");

            let output = Command::new("ip")
                .args(["addr", "add", &addr_str, "dev", interface])
                .output()
                .await
                .map_err(|e| NetworkError::Io(format!("failed to run ip: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(NetworkError::ConfigurationFailed(format!(
                    "ip addr add failed: {stderr}"
                )));
            }
        }

        if let Some(ref gateway) = config.gateway {
            let output = Command::new("ip")
                .args(["route", "add", "default", "via", &gateway.to_string()])
                .output()
                .await
                .map_err(|e| NetworkError::Io(format!("failed to add route: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.contains("EEXIST") {
                    return Err(NetworkError::ConfigurationFailed(format!(
                        "ip route add failed: {stderr}"
                    )));
                }
            }
        }

        if config.dhcp {
            let output = Command::new("dhclient")
                .arg(interface)
                .output()
                .await
                .map_err(|e| NetworkError::Io(format!("failed to run dhclient: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(NetworkError::ConfigurationFailed(format!(
                    "dhclient failed: {stderr}"
                )));
            }
        }

        Ok(())
    }

    async fn dns_lookup(
        &self,
        _ctx: &CapabilityContext,
        host: &str,
    ) -> Result<IpAddr, NetworkError> {
        let host = host.to_string();

        let ips = tokio::task::spawn_blocking(move || {
            (host.as_str(), 0u16)
                .to_socket_addrs()
                .map(|iter| iter.map(|s| s.ip()).collect::<Vec<_>>())
        })
        .await
        .map_err(|e| NetworkError::Io(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| NetworkError::DnsFailed(e.to_string()))?;

        ips.into_iter()
            .next()
            .ok_or_else(|| NetworkError::DnsFailed("no addresses found".to_string()))
    }

    async fn set_hostname(
        &self,
        _ctx: &CapabilityContext,
        hostname: &str,
    ) -> Result<(), NetworkError> {
        let output = Command::new("hostnamectl")
            .arg("set-hostname")
            .arg(hostname)
            .output()
            .await
            .map_err(|e| NetworkError::Io(format!("failed to execute hostnamectl: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(NetworkError::ConfigurationFailed(format!(
                "hostnamectl failed: {stderr}"
            )))
        }
    }

    fn events(&self) -> Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}
