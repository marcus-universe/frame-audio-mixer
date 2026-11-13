//! Where the headset might be: a fixed address from the settings, or every
//! local network's broadcast address.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

/// Addresses to ask for the headset. Called before every probe, so a network
/// adapter that comes up later (the Steam Frame's own wireless adapter only
/// connects while Steam streams) is picked up.
#[must_use]
pub fn candidates(address: &str, port: u16) -> Vec<SocketAddr> {
    let fixed = fixed_address(address, port);
    if !fixed.is_empty() {
        return fixed;
    }
    let mut out = Vec::new();
    let Ok(interfaces) = if_addrs::get_if_addrs() else { return out };
    for interface in interfaces {
        if interface.is_loopback() {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = interface.addr else { continue };
        let broadcast = v4.broadcast.unwrap_or_else(|| broadcast_of(v4.ip, v4.netmask));
        let target = SocketAddr::new(IpAddr::V4(broadcast), port);
        if !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

/// The address the user typed, with or without a port. A name is resolved.
fn fixed_address(address: &str, port: u16) -> Vec<SocketAddr> {
    let address = address.trim();
    if address.is_empty() {
        return Vec::new();
    }
    if let Ok(ip) = address.parse::<IpAddr>() {
        return vec![SocketAddr::new(ip, port)];
    }
    let with_port = if address.contains(':') { address.to_string() } else { format!("{address}:{port}") };
    with_port.to_socket_addrs().map(|found| found.filter(SocketAddr::is_ipv4).collect()).unwrap_or_default()
}

fn broadcast_of(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) | !u32::from(netmask))
}

/// This computer's name, shown in the headset mixer.
#[must_use]
pub fn host_name() -> String {
    let name = if cfg!(windows) {
        std::env::var("COMPUTERNAME").ok()
    } else {
        std::fs::read_to_string("/proc/sys/kernel/hostname").ok().or_else(|| std::env::var("HOSTNAME").ok())
    };
    let name = name.map(|name| name.trim().to_string()).filter(|name| !name.is_empty());
    name.unwrap_or_else(|| "PC".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_ip_should_be_the_only_candidate() {
        let found = candidates("10.35.78.1", 47_811);

        assert_eq!(found, vec![SocketAddr::from(([10, 35, 78, 1], 47_811))]);
    }

    #[test]
    fn fixed_ip_with_a_port_should_keep_its_port() {
        let found = candidates(" 10.35.78.1:5000 ", 47_811);

        assert_eq!(found, vec![SocketAddr::from(([10, 35, 78, 1], 5000))]);
    }

    #[test]
    fn broadcast_should_fill_the_host_bits() {
        let broadcast = broadcast_of(Ipv4Addr::new(192, 168, 50, 78), Ipv4Addr::new(255, 255, 255, 0));

        assert_eq!(broadcast, Ipv4Addr::new(192, 168, 50, 255));
    }

    #[test]
    fn host_name_should_never_be_empty() {
        assert!(!host_name().is_empty());
    }
}
