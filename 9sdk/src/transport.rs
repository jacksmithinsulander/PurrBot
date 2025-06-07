use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite};
use std::pin::Pin;
use std::io;

#[cfg(feature = "vsock")]
use vsock::{VsockListener, VsockStream};

#[derive(Clone)]
pub enum Transport {
    Tcp(SocketAddr),
    #[cfg(feature = "vsock")]
    Vsock(u32, u32),  // (cid, port)
}

pub trait TransportStream: AsyncRead + AsyncWrite + Send {}

impl TransportStream for TcpStream {}

#[cfg(feature = "vsock")]
impl TransportStream for VsockStream {}

pub async fn connect(transport: Transport) -> io::Result<Pin<Box<dyn TransportStream>>> {
    match transport {
        Transport::Tcp(addr) => {
            let stream = TcpStream::connect(addr).await?;
            Ok(Box::pin(stream))
        }
        #[cfg(feature = "vsock")]
        Transport::Vsock(cid, port) => {
            let stream = VsockStream::connect(cid, port).await?;
            Ok(Box::pin(stream))
        }
    }
}

pub async fn listen(transport: Transport) -> io::Result<Pin<Box<dyn TransportStream>>> {
    match transport {
        Transport::Tcp(addr) => {
            let listener = TcpListener::bind(addr).await?;
            let (stream, _) = listener.accept().await?;
            Ok(Box::pin(stream))
        }
        #[cfg(feature = "vsock")]
        Transport::Vsock(cid, port) => {
            let listener = VsockListener::bind(cid, port)?;
            let (stream, _) = listener.accept().await?;
            Ok(Box::pin(stream))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_connect() {
        let transport = Transport::Tcp("127.0.0.1:5005".parse().unwrap());
        let result = connect(transport).await;
        // This will fail to connect, but we just want to verify it compiles
        assert!(result.is_err());
    }
} 