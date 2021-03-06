use futures::{failed, Future};
use tokio_core::{LoopHandle, TcpListener, TcpStream, UdpSocket};
use tokio_core::io::IoFuture;

use std::io;
use std::net::{IpAddr, SocketAddr};

use super::select_all_ok::select_all_ok;
use super::{Endpoint, Resolver, ToEndpoint};

pub fn tcp_connect_par<'a, T, R>(handle: LoopHandle, resolver: R, ep: T) -> IoFuture<TcpStream>
    where R: Resolver,
          T: ToEndpoint<'a>,

{
    if_host_resolve(handle, resolver, ep, |handle, port, ip_addrs| {
        debug!("creating {} parallel connection attemps", ip_addrs.len());

        let futs = ip_addrs.into_iter().map(|ip_addr| {
            let addr = SocketAddr::new(ip_addr, port);
            handle.clone().tcp_connect(&addr)
        });

        select_all_ok(futs).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "all of the connections attempts failed")
        }).boxed()
    }, |handle, addr| handle.tcp_connect(addr))
}

pub fn tcp_connect_seq<'a, R, T>(handle: LoopHandle, resolver: R, ep: T) -> IoFuture<TcpStream>
    where R: Resolver,
          T: ToEndpoint<'a>
{
    if_host_resolve(handle, resolver, ep, |handle, port, ip_addrs| {
        debug!("chaining {} connection attempts", ip_addrs.len());

        let mut prev: Option<IoFuture<TcpStream>> = None;

        // This loop chains futures one after another so they each try
        // to connect to an address in a sequential way.
        for ip_addr in ip_addrs {
            let addr = SocketAddr::new(ip_addr, port);
            let handle = handle.clone();

            prev = Some(match prev.take() {
                None => handle.tcp_connect(&addr).boxed(),
                Some(prev) => prev.or_else(move |_| handle.tcp_connect(&addr)).boxed(),
            });
        }

        // If this Option is None, it means that there were no addresses in the list.
        match prev.take() {
            Some(fut) => fut,
            None => failed(io::Error::new(io::ErrorKind::Other, "resolve returned no addresses")).boxed(),
        }
    }, |handle, addr| handle.tcp_connect(addr))
}

pub fn tcp_listen_seq<'a, R, T>(handle: LoopHandle, resolver: R, ep: T) -> IoFuture<TcpListener>
    where R: Resolver,
          T: ToEndpoint<'a>
{
    if_host_resolve(handle, resolver, ep, |handle, port, ip_addrs| {
        debug!("chaining {} connection attempts", ip_addrs.len());

        let mut prev: Option<IoFuture<TcpListener>> = None;

        // This loop chains futures one after another so they each try
        // to connect to an address in a sequential way.
        for ip_addr in ip_addrs {
            let addr = SocketAddr::new(ip_addr, port);
            let handle = handle.clone();

            prev = Some(match prev.take() {
                None => handle.tcp_listen(&addr).boxed(),
                Some(prev) => prev.or_else(move |_| handle.tcp_listen(&addr)).boxed(),
            });
        }

        // If this Option is None, it means that there were no addresses in the list.
        match prev.take() {
            Some(fut) => fut,
            None => failed(io::Error::new(io::ErrorKind::Other, "resolve returned no addresses")).boxed(),
        }
    }, |handle, addr| handle.tcp_listen(addr))
}

pub fn udp_bind_seq<'a, R, T>(handle: LoopHandle, resolver: R, ep: T) -> IoFuture<UdpSocket>
    where R: Resolver,
          T: ToEndpoint<'a>
{
    if_host_resolve(handle, resolver, ep, |handle, port, ip_addrs| {
        debug!("chaining {} connection attempts", ip_addrs.len());

        let mut prev: Option<IoFuture<UdpSocket>> = None;

        // This loop chains futures one after another so they each try
        // to connect to an address in a sequential way.
        for ip_addr in ip_addrs {
            let addr = SocketAddr::new(ip_addr, port);
            let handle = handle.clone();

            prev = Some(match prev.take() {
                None => handle.udp_bind(&addr).boxed(),
                Some(prev) => prev.or_else(move |_| handle.udp_bind(&addr)).boxed(),
            });
        }

        // If this Option is None, it means that there were no addresses in the list.
        match prev.take() {
            Some(fut) => fut,
            None => failed(io::Error::new(io::ErrorKind::Other, "resolve returned no addresses")).boxed(),
        }
    }, |handle, addr| handle.udp_bind(addr))
}

// abstraction of the code that is common to tcp_connect_(par|seq).
fn if_host_resolve<'a, R, T, F, E, S>(handle: LoopHandle, resolver: R, ep: T, func: F, elsef: E) -> IoFuture<S>
        where R: Resolver,
              T: ToEndpoint<'a>,
              F: FnOnce(LoopHandle, u16, Vec<IpAddr>) -> IoFuture<S> + Send + 'static,
              E: FnOnce(LoopHandle, &SocketAddr) -> IoFuture<S> + Send + 'static,
              S: Send + 'static,
{
    let ep = match ep.to_endpoint() {
        Ok(ep) => ep,
        Err(e) => return failed(e).boxed(),
    };

    match ep {
        Endpoint::Host(host, port) => {
            resolver.resolve(host).and_then(move |addrs| {
                func(handle, port, addrs)
            }).boxed()
        }
        Endpoint::SocketAddr(ref addr) => {
            elsef(handle, addr)
        }
    }
}
