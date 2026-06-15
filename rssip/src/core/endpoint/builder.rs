use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::{io, mem};

use crate::core::endpoint::EndpointInner;
use crate::core::endpoint::plugin::{Plugin, Plugins};
use crate::message::headers::{Accept, Allow, Header, Supported};
use crate::message::method::SipMethod;
use crate::transport::tcp::TcpListener;
use crate::transport::udp::UdpTransport;
use crate::transport::ws::WebSocketListener;
use crate::transport::{SipTransport, TransportHandle, TransportLayer};
use crate::{Endpoint, MediaType, Result};

/// Builder for creating a new SIP `Endpoint`.
pub struct EndpointBuilder {
    name: String,
    plugins: Plugins,
    allow: Allow,
    accept: Accept,
    supported: Supported,
    tcp: Vec<Box<dyn AddressResolver>>,
    udp: Vec<Box<dyn AddressResolver>>,
    ws: Vec<Box<dyn AddressResolver>>,
}

pub trait AddressResolver: Send + Sync {
    fn resolve(&self) -> io::Result<Vec<SocketAddr>>;
}

impl<T> AddressResolver for T
where
    T: ToSocketAddrs + Clone + Send + Sync + 'static,
{
    fn resolve(&self) -> io::Result<Vec<SocketAddr>> {
        self.to_socket_addrs().map(|iter| iter.collect())
    }
}

impl EndpointBuilder {
    pub fn new() -> Self {
        EndpointBuilder {
            name: String::new(),
            plugins: Plugins::default(),
            accept: Accept::default(),
            allow: Allow::default(),
            supported: Supported::default(),
            tcp: Default::default(),
            udp: Default::default(),
            ws: Default::default(),
        }
    }

    pub fn with_tcp_addr<A>(mut self, addr: A) -> EndpointBuilder
    where
        A: AddressResolver + 'static,
    {
        self.tcp.push(Box::new(addr));

        self
    }

    pub fn with_udp_addr<A>(mut self, addr: A) -> EndpointBuilder
    where
        A: AddressResolver + 'static,
    {
        self.udp.push(Box::new(addr));
        self
    }

    pub fn with_ws_addr<A>(mut self, addr: A) -> EndpointBuilder
    where
        A: AddressResolver + 'static,
    {
        self.ws.push(Box::new(addr));
        self
    }

    pub fn with_name(mut self, name: String) -> EndpointBuilder {
        self.name = name;

        self
    }

    pub fn with_plugin<M: Plugin>(mut self, plugin: M) -> EndpointBuilder {
        self.plugins.add_plugin(plugin);

        self
    }

    pub fn insert_allow(&mut self, sip_method: SipMethod) {
        self.allow.push(sip_method);
    }

    pub fn insert_accept(&mut self, media_type: MediaType) {
        self.accept.push(media_type);
    }

    pub fn insert_supported(&mut self, tag: String) {
        self.supported.add_tag(tag);
    }

    /// Finalize the Builder into a `Endpoint`.
    pub async fn build(mut self) -> Result<Endpoint> {
        log::trace!("Creating endpoint...");

        let mut plugins = std::mem::take(&mut self.plugins);

        for plugin in plugins.iter_mut() {
            plugin.on_load(&mut self);
            log::debug!("Plugin {} loaded", format_args!("({})", plugin.name()));
        }

        let endpoint = {
            let endpoint = Arc::new_cyclic(|me: &std::sync::Weak<EndpointInner>| {
                let handle = super::WeakEndpointHandle(me.clone());
                let transport = TransportLayer::new(handle);

                EndpointInner {
                    transport,
                    name: self.name,
                    allow: self.allow,
                    supported: self.supported,
                    accept: self.accept,
                    plugins,
                }
            });

            Endpoint { inner: endpoint }
        };

        let mut tcp_resolvers = mem::take(&mut self.tcp);
        let mut udp_resolvers = mem::take(&mut self.udp);
        let mut ws_resolvers = mem::take(&mut self.ws);

        let transports = &endpoint.inner.transport;

        while let Some(resolver) = tcp_resolvers.pop() {
            for addr in resolver.resolve()? {
                let tcp = TcpListener::bind(addr).await?;
                log::info!(
                    "SIP TCP listener ready for incoming connections at: {}",
                    tcp.local_addr()
                );
                tokio::spawn(tcp.accept_clients(transports.clone()));
            }
        }

        while let Some(resolver) = udp_resolvers.pop() {
            for addr in resolver.resolve()? {
                let udp = UdpTransport::bind(addr).await?;
                log::info!("SIP UDP transport started, bound to: {}", udp.local_addr());
                transports.register_transport(udp.key(), TransportHandle::new(udp.clone()));

                tokio::spawn(udp.receive_datagram(transports.clone()));
            }
        }

        while let Some(resolver) = ws_resolvers.pop() {
            for addr in resolver.resolve()? {
                let ws = WebSocketListener::bind(addr).await?;
                log::info!(
                    "SIP WS listener ready for incoming connections at: {}",
                    ws.local_addr()
                );
                tokio::spawn(ws.accept_clients(transports.clone()));
            }
        }

        Ok(endpoint)
    }
}

impl Default for EndpointBuilder {
    fn default() -> Self {
        Self::new()
    }
}
