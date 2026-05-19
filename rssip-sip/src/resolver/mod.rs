//! DNS resolve with the `SipDomainResolver` type.

use std::net::SocketAddr;
use std::{io, ops, sync};

use tokio::net;
use utils::OneOrMore;

use crate::message::sip_uri::{Host, HostPort};
use crate::transport::TransportProtocol;

type IoResult<T> = io::Result<T>;

#[async_trait::async_trait]
pub trait SipDomainResolver: Send + Sync + 'static {
    async fn resolve(&self, target: &SipHost) -> IoResult<ServerAddresses>;
}

pub struct DefaultResolver;

#[async_trait::async_trait]
impl SipDomainResolver for DefaultResolver {
    async fn resolve(&self, target: &SipHost) -> IoResult<ServerAddresses> {
        let protocol = match target.protocol {
            Some(protocol) => protocol,
            None => TransportProtocol::Udp,
        };
        let port = match target.host_port.port {
            Some(port) => port,
            None => protocol.default_port(),
        };
        let socket_addr = match target.host_port.host {
            Host::HostName(ref host_name) => {
                let host = format!("{}:{}", host_name, port);

                let mut iter_addr = net::lookup_host(host).await?;

                let Some(addr) = iter_addr.next() else {
                    return Err(io::Error::other(format!(
                        "No address found for '{host_name}'"
                    )));
                };

                addr
            }
            Host::IpAddr(ip_addr) => SocketAddr::new(ip_addr, port),
        };

        let addresses = OneOrMore::one(LookupAddress {
            socket_addr,
            protocol,
        });

        Ok(ServerAddresses { addresses })
    }
}

#[derive(Clone)]
pub struct DomainResolver(sync::Arc<dyn SipDomainResolver>);

impl<T: SipDomainResolver> From<T> for DomainResolver {
    fn from(value: T) -> Self {
        Self(sync::Arc::new(value))
    }
}

impl ops::Deref for DomainResolver {
    type Target = dyn SipDomainResolver;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

#[derive(Debug)]
pub struct SipHost {
    pub host_port: HostPort,
    pub protocol: Option<TransportProtocol>,
}

pub struct LookupAddress {
    pub socket_addr: SocketAddr,
    pub protocol: TransportProtocol,
}

pub struct ServerAddresses {
    addresses: OneOrMore<LookupAddress>,
}

impl ServerAddresses {
    pub fn new(addresses: OneOrMore<LookupAddress>) -> Self {
        Self { addresses }
    }
}

impl From<OneOrMore<LookupAddress>> for ServerAddresses {
    fn from(value: OneOrMore<LookupAddress>) -> Self {
        Self::new(value)
    }
}

impl IntoIterator for ServerAddresses {
    type Item = LookupAddress;

    type IntoIter = utils::one::IntoIter<LookupAddress>;

    fn into_iter(self) -> Self::IntoIter {
        self.addresses.into_iter()
    }
}
