use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};

#[cfg(feature = "vsock")]
use vsock::{VsockListener, VsockStream, VsockAddr};

/// Transport configuration for different connection types
#[derive(Clone)]
pub enum Transport {
    Tcp(SocketAddr),
    #[cfg(feature = "vsock")]
    Vsock { cid: u32, port: u32 },
}

/// Trait for unified stream handling across transport types
pub trait TransportStream: AsyncRead + AsyncWrite + Send {}

impl TransportStream for TcpStream {}

/// Wrapper for VsockStream to implement async traits
#[cfg(feature = "vsock")]
pub struct AsyncVsockStream {
    inner: VsockStream,
}

#[cfg(feature = "vsock")]
impl AsyncRead for AsyncVsockStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        use std::io::Read;
        let mut temp_buf = vec![0u8; buf.remaining()];
        match self.inner.read(&mut temp_buf) {
            Ok(n) => {
                buf.put_slice(&temp_buf[..n]);
                std::task::Poll::Ready(Ok(()))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::task::Poll::Pending,
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

#[cfg(feature = "vsock")]
impl AsyncWrite for AsyncVsockStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        use std::io::Write;
        match self.inner.write(buf) {
            Ok(n) => std::task::Poll::Ready(Ok(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::task::Poll::Pending,
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        use std::io::Write;
        match self.inner.flush() {
            Ok(()) => std::task::Poll::Ready(Ok(())),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::task::Poll::Pending,
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "vsock")]
impl TransportStream for AsyncVsockStream {}

/// Creates a listener and accepts one connection (for backward compatibility)
pub async fn listen(transport: Transport) -> io::Result<Pin<Box<dyn TransportStream>>> {
    match transport {
        Transport::Tcp(addr) => listen_tcp(addr).await,
        #[cfg(feature = "vsock")]
        Transport::Vsock { cid, port } => listen_vsock(cid, port).await,
    }
}

/// Establishes a client connection using the specified transport
pub async fn connect(transport: Transport) -> io::Result<Pin<Box<dyn TransportStream>>> {
    match transport {
        Transport::Tcp(addr) => connect_tcp(addr).await,
        #[cfg(feature = "vsock")]
        Transport::Vsock { cid, port } => connect_vsock(cid, port).await,
    }
}

// Private helper functions for better separation of concerns

async fn connect_tcp(addr: SocketAddr) -> io::Result<Pin<Box<dyn TransportStream>>> {
    let stream = TcpStream::connect(addr).await?;
    Ok(Box::pin(stream))
}

async fn listen_tcp(addr: SocketAddr) -> io::Result<Pin<Box<dyn TransportStream>>> {
    let listener = TcpListener::bind(addr).await?;
    let (stream, _) = listener.accept().await?;
    Ok(Box::pin(stream))
}

#[cfg(feature = "vsock")]
async fn connect_vsock(cid: u32, port: u32) -> io::Result<Pin<Box<dyn TransportStream>>> {
    use std::os::unix::io::AsRawFd;
    
    // Create VsockAddr and connect
    let addr = VsockAddr::new(cid, port);
    let stream = VsockStream::connect(&addr)?;
    
    // Set non-blocking mode for async operation
    let fd = stream.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    
    let async_stream = AsyncVsockStream { inner: stream };
    Ok(Box::pin(async_stream))
}

#[cfg(feature = "vsock")]
async fn listen_vsock(cid: u32, port: u32) -> io::Result<Pin<Box<dyn TransportStream>>> {
    use std::os::unix::io::AsRawFd;
    
    // Create VsockAddr and bind listener
    let addr = VsockAddr::new(cid, port);
    let listener = VsockListener::bind(&addr)?;
    let (stream, _) = listener.accept()?;
    
    // Set non-blocking mode for async operation
    let fd = stream.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    
    let async_stream = AsyncVsockStream { inner: stream };
    Ok(Box::pin(async_stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::time::Duration;
    use tokio::time::timeout;

    // Test constants
    const TEST_PORT: u16 = 12345;
    const LOCALHOST_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    const LOCALHOST_V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));

    // Transport creation tests

    #[test]
    fn test_create_tcp_transport() {
        let addr = SocketAddr::new(LOCALHOST_V4, TEST_PORT);
        let transport = Transport::Tcp(addr);
        
        match transport {
            Transport::Tcp(socket_addr) => {
                assert_eq!(socket_addr, addr);
                assert_eq!(socket_addr.port(), TEST_PORT);
                assert_eq!(socket_addr.ip(), LOCALHOST_V4);
            }
            #[cfg(feature = "vsock")]
            _ => panic!("Expected TCP transport"),
        }
    }

    #[test]
    fn test_create_tcp_transport_ipv6() {
        let addr = SocketAddr::new(LOCALHOST_V6, TEST_PORT);
        let transport = Transport::Tcp(addr);
        
        match transport {
            Transport::Tcp(socket_addr) => {
                assert_eq!(socket_addr.ip(), LOCALHOST_V6);
                assert_eq!(socket_addr.port(), TEST_PORT);
            }
            #[cfg(feature = "vsock")]
            _ => panic!("Expected TCP transport"),
        }
    }

    #[test]
    fn test_transport_clone() {
        let addr = SocketAddr::new(LOCALHOST_V4, TEST_PORT);
        let transport1 = Transport::Tcp(addr);
        let transport2 = transport1.clone();
        
        match (transport1, transport2) {
            (Transport::Tcp(addr1), Transport::Tcp(addr2)) => {
                assert_eq!(addr1, addr2);
            }
            #[cfg(feature = "vsock")]
            _ => panic!("Expected TCP transports"),
        }
    }

    #[cfg(feature = "vsock")]
    #[test]
    fn test_create_vsock_transport() {
        let cid = 3;
        let port = 5000;
        let transport = Transport::Vsock { cid, port };
        
        match transport {
            Transport::Vsock { cid: c, port: p } => {
                assert_eq!(c, cid);
                assert_eq!(p, port);
            }
            _ => panic!("Expected Vsock transport"),
        }
    }

    // TCP connection tests

    #[tokio::test]
    async fn test_tcp_connect_to_invalid_address() {
        let addr = SocketAddr::new(LOCALHOST_V4, 1); // Port 1 should be unavailable
        let transport = Transport::Tcp(addr);
        
        let result = connect(transport).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tcp_listen_on_available_port() {
        let addr = SocketAddr::new(LOCALHOST_V4, 0); // Port 0 = any available port
        let transport = Transport::Tcp(addr);
        
        // Should succeed as port 0 lets the OS assign an available port
        let result = timeout(Duration::from_secs(1), listen(transport)).await;
        
        // Timeout is expected since no one connects
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tcp_connection_lifecycle() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        // Use port 0 to get an available port
        let listener_addr = SocketAddr::new(LOCALHOST_V4, 0);
        let listener = TcpListener::bind(listener_addr).await.unwrap();
        let actual_addr = listener.local_addr().unwrap();
        
        // Spawn listener task
        let listener_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            // Read data
            let mut buf = [0u8; 5];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"Hello");
            
            // Write response
            stream.write_all(b"World").await.unwrap();
        });
        
        // Give listener time to start
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Connect to listener
        let transport = Transport::Tcp(actual_addr);
        let mut stream = connect(transport).await.unwrap();
        
        // Write data
        stream.write_all(b"Hello").await.unwrap();
        
        // Read response
        let mut buf = [0u8; 5];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"World");
        
        // Wait for listener task
        listener_task.await.unwrap();
    }

    // Helper function tests

    #[tokio::test]
    async fn test_connect_tcp_invalid_address() {
        let addr = SocketAddr::new(LOCALHOST_V4, 1);
        let result = connect_tcp(addr).await;
        assert!(result.is_err());
    }

    // Vsock tests (only compiled when feature is enabled)

    #[cfg(feature = "vsock")]
    #[tokio::test]
    async fn test_vsock_connect_invalid() {
        let transport = Transport::Vsock { cid: 999999, port: 12345 };
        let result = connect(transport).await;
        assert!(result.is_err());
    }

    #[cfg(feature = "vsock")]
    #[test]
    fn test_vsock_transport_clone() {
        let transport1 = Transport::Vsock { cid: 3, port: 5000 };
        let transport2 = transport1.clone();
        
        match (transport1, transport2) {
            (Transport::Vsock { cid: c1, port: p1 }, Transport::Vsock { cid: c2, port: p2 }) => {
                assert_eq!(c1, c2);
                assert_eq!(p1, p2);
            }
            _ => panic!("Expected Vsock transports"),
        }
    }

    // Test transport stream trait implementation

    #[tokio::test]
    async fn test_transport_stream_trait_implementation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0; 4];
            stream.read_exact(&mut buf).await.unwrap();
            stream.write_all(&buf).await.unwrap();
        });
        
        let transport = Transport::Tcp(addr);
        let mut stream: Pin<Box<dyn TransportStream>> = connect(transport).await.unwrap();
        
        // Test that TransportStream trait works correctly
        stream.write_all(b"test").await.unwrap();
        let mut buf = vec![0; 4];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"test");
    }
}
