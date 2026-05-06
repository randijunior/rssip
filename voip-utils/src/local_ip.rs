use std::net::{IpAddr, Ipv4Addr};

/// Retrieve system local IP address.
pub fn get_local_ip_addr() -> IpAddr {
    local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}
