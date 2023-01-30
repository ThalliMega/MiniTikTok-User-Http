use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use hyper::server::{accept::Accept, conn::AddrIncoming};

pub struct CombinedIncoming {
    a: AddrIncoming,
    b: AddrIncoming,
}

impl Accept for CombinedIncoming {
    type Conn = <AddrIncoming as Accept>::Conn;
    type Error = <AddrIncoming as Accept>::Error;
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        if let Poll::Ready(value) = Pin::new(&mut self.a).poll_accept(cx) {
            return Poll::Ready(value);
        }

        if let Poll::Ready(value) = Pin::new(&mut self.b).poll_accept(cx) {
            return Poll::Ready(value);
        }

        Poll::Pending
    }
}

impl CombinedIncoming {
    pub fn new(a: &SocketAddr, b: &SocketAddr) -> Result<Self, hyper::Error> {
        Ok(Self {
            a: AddrIncoming::bind(a)?,
            b: AddrIncoming::bind(b)?,
        })
    }
}
