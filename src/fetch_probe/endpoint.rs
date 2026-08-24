use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use anyhow::{Context, bail};

pub(crate) trait EndpointResolver: Send + Sync {
    fn resolve(&self, host: &str, port: u16) -> anyhow::Result<IpAddr>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemResolver;

impl EndpointResolver for SystemResolver {
    fn resolve(&self, host: &str, port: u16) -> anyhow::Result<IpAddr> {
        let mut addresses = (host, port)
            .to_socket_addrs()
            .with_context(|| format!("fetch DNS resolution failed for {host}"))?
            .map(|address| address.ip())
            .collect::<Vec<_>>();
        addresses.sort();
        addresses.dedup();
        if addresses.is_empty() {
            bail!("fetch DNS resolution returned no addresses for {host}");
        }
        if let Some(unsafe_ip) = addresses.iter().find(|ip| !is_public_ip(**ip)) {
            bail!("fetch DNS resolution returned unsafe address {unsafe_ip}");
        }
        Ok(addresses[0])
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FixedResolver(pub IpAddr);

impl EndpointResolver for FixedResolver {
    fn resolve(&self, _host: &str, _port: u16) -> anyhow::Result<IpAddr> {
        if !is_public_ip(self.0) {
            bail!("recorded resolver address is unsafe");
        }
        Ok(self.0)
    }
}

pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 198 && (18..=19).contains(&b)))
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_v4(ipv4);
    }
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || segments[0] & 0xffc0 == 0xfec0
        || segments[..6].iter().all(|segment| *segment == 0)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_local_private_documentation_and_shared_addresses() {
        for raw in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "100.64.0.1",
            "198.18.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "::127.0.0.1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!is_public_ip(raw.parse().unwrap()), "{raw}");
        }
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
