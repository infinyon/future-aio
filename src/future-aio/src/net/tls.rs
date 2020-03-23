use crate::net::TcpStream;

pub use async_tls::server::TlsStream as ServerTlsStream;
pub use async_tls::client::TlsStream as ClientTlsStream;
pub use async_tls::TlsAcceptor;
pub use async_tls::TlsConnector;

pub type DefaultServerTlsStream = ServerTlsStream<TcpStream>;
pub type DefaultClientTlsStream = ClientTlsStream<TcpStream>;



use rustls::ClientConfig;
use rustls::Certificate;
use rustls::PrivateKey;
use rustls::ServerConfig;
use rustls::RootCertStore;

pub use cert::*;
pub use connector::*;
pub use builder::*;

mod cert {
    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::io::BufReader;
    use std::fs::File;
    
    use rustls::internal::pemfile::certs;
    use rustls::internal::pemfile::rsa_private_keys;

    
    use super::Certificate;
    use super::PrivateKey;
    use super::RootCertStore;

    pub fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<Certificate>,IoError> {
        certs(&mut BufReader::new(File::open(path)?))
            .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))
    }

    /// Load the passed keys file
    pub fn load_keys<P: AsRef<Path>>(path: P) -> Result<Vec<PrivateKey>,IoError> {
        rsa_private_keys(&mut BufReader::new(File::open(path)?))
            .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid key"))
    }

    pub fn load_root_ca<P: AsRef<Path>>(path: P) -> Result<RootCertStore,IoError> {

        let mut root_store = RootCertStore::empty();

        root_store
            .add_pem_file(&mut BufReader::new(File::open(path)?))
            .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid ca crt"))?;

        Ok(root_store)

    }

    
}

mod connector {

    use std::io::Error as IoError;

    #[cfg(unix)]
    use std::os::unix::io::RawFd;
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;

    use log::debug;
    use async_trait::async_trait;

    use super::TlsConnector;
    use super::super::TcpDomainConnector;
    use super::super::DefaultTcpDomainConnector;
    use super::DefaultClientTlsStream;
    use super::TcpStream;
    use super::AllTcpStream;

    /// connect as anonymous client
    #[derive(Clone)]
    pub struct TlsAnonymousConnector(TlsConnector);

    impl From<TlsConnector> for TlsAnonymousConnector {
        fn from(connector: TlsConnector) -> Self {
            Self(connector)
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsAnonymousConnector {

        type WrapperStream =  DefaultClientTlsStream;

        async fn connect(&self,domain: &str) -> Result<(Self::WrapperStream,RawFd),IoError>  {
            let tcp_stream = TcpStream::connect(domain).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((self.0.connect(domain, tcp_stream).await?,fd))
        }
    }



    #[derive(Clone)]
    pub struct TlsDomainConnector {
        domain: String,
        connector: TlsConnector
    }

    impl TlsDomainConnector {
        pub fn new(connector: TlsConnector,domain: String) -> Self {
            Self {
                domain,
                connector
            }
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsDomainConnector {

        type WrapperStream =  DefaultClientTlsStream;

        async fn connect(&self,addr: &str) -> Result<(Self::WrapperStream,RawFd),IoError>  {
            debug!("connect to tls addr: {}",addr);
            let tcp_stream = TcpStream::connect(addr).await?;
            let fd = tcp_stream.as_raw_fd();

            debug!("connect to tls domain: {}",self.domain);
            Ok((self.connector.connect(&self.domain, tcp_stream).await?,fd))
        }
    }

    


    #[derive(Clone)]
    pub enum AllDomainConnector {
        Tcp(DefaultTcpDomainConnector),
        TlsDomain(TlsDomainConnector),
        TlsAnonymous(TlsAnonymousConnector)
    }

    impl Default for AllDomainConnector {
        fn default() -> Self {
            Self::default_tcp()
        }
    }


    impl AllDomainConnector {

        pub fn default_tcp() -> Self {
            Self::Tcp(DefaultTcpDomainConnector::new())
        }

        pub fn new_tls_domain(connector: TlsDomainConnector) -> Self {
            Self::TlsDomain(connector)
        }

        pub fn new_tls_anonymous(connector: TlsAnonymousConnector) -> Self {
            Self::TlsAnonymous(connector)
        }

    }

    
    #[async_trait]
    impl TcpDomainConnector for AllDomainConnector  {

        type WrapperStream = AllTcpStream;

        async fn connect(&self,domain: &str) -> Result<(Self::WrapperStream,RawFd),IoError> {  

            match self {
                Self::Tcp(connector) => { 
                    let (stream,fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tcp(stream),fd))
                },
                
                Self::TlsDomain(connector) => {
                    let (stream,fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tls(stream),fd))
                },
                Self::TlsAnonymous(connector) => {
                    let (stream,fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tls(stream),fd))
                }
                
            }

        }

    }

}

mod builder {

    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::sync::Arc;
    use std::io::BufReader;
    use std::fs::File;

    use rustls::ServerCertVerifier;
    use rustls::ServerCertVerified;
    use rustls::TLSError;
    use webpki::DNSNameRef;
    use rustls::AllowAnyAuthenticatedClient;

    use super::ClientConfig;
    use super::load_certs;
    use super::load_keys;
    use super::TlsConnector;
    use super::ServerConfig;
    use super::load_root_ca;
    use super::TlsAcceptor;
    use super::RootCertStore;
    use super::Certificate;

    pub struct ConnectorBuilder(ClientConfig);

    impl ConnectorBuilder {

        pub fn new() -> Self {
            Self(ClientConfig::new())
        }

        pub fn load_ca_cert<P: AsRef<Path>>(mut self,path: P) -> Result<Self,IoError>  {

            self.0.root_store
                .add_pem_file(&mut BufReader::new(File::open(path)?))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid ca crt"))?;

            Ok(self)
        }

        pub fn load_client_certs<P: AsRef<Path>>(
            mut self,
            cert_path: P,
            key_path: P,
        ) -> Result<Self,IoError> {


            let client_certs = load_certs(cert_path)?;
            let mut client_keys = load_keys(key_path)?;
            self.0
                .set_single_client_cert(client_certs,client_keys.remove(0))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))?;
            
            Ok(self)
        }

        
        pub fn no_cert_verification(mut self) -> Self {

            self.0
                .dangerous()
                .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

            self
        }

        pub fn build(self) -> TlsConnector {
            
            TlsConnector::from(Arc::new(self.0))
        }
    }

    pub struct AcceptorBuilder(ServerConfig);

    impl AcceptorBuilder {

        /// create builder with no client authentication
        pub fn new_no_client_authentication() -> Self {
            use rustls::NoClientAuth;

            Self(ServerConfig::new(NoClientAuth::new()))
        }

        /// create builder with client authentication
        /// must pass CA root
        pub fn new_client_authenticate<P: AsRef<Path>>(path: P) -> Result<Self,IoError> {

            let root_store = load_root_ca(path)?;
            
            Ok(Self(ServerConfig::new(AllowAnyAuthenticatedClient::new(root_store))))
        }

        pub fn load_server_certs<P: AsRef<Path>>(
            mut self,
            cert_path: P,
            key_path: P,
        ) -> Result<Self,IoError> {


            let server_crt = load_certs(cert_path)?;
            let mut server_keys = load_keys(key_path)?;
            self.0
                .set_single_cert(server_crt,server_keys.remove(0))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))?;
            
            Ok(self)
        }

        pub fn build(self) -> TlsAcceptor {
            
            TlsAcceptor::from(Arc::new(self.0))
        }

    }

    struct NoCertificateVerification {}

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(&self,
                            _roots: &RootCertStore,
                            _presented_certs: &[Certificate],
                            _dns_name: DNSNameRef<'_>,
                            _ocsp: &[u8]) -> Result<ServerCertVerified,TLSError> {

            log::debug!("ignoring server cert");
            Ok(ServerCertVerified::assertion())
        }
    }



}



pub use stream::AllTcpStream;

mod stream {

    use std::pin::Pin;
    use std::io;
    use std::task::{Context, Poll};

    use pin_project::{pin_project, project};

    use super::TcpStream;
    use super::DefaultClientTlsStream;

    #[pin_project]
    pub enum AllTcpStream {
        Tcp(#[pin] TcpStream),
        Tls(#[pin] DefaultClientTlsStream)
    }

    impl AllTcpStream  {
        pub fn tcp(stream: TcpStream) -> Self {
            Self::Tcp(stream)
        }

        pub fn tls(stream: DefaultClientTlsStream) -> Self {
            Self::Tls(stream)
        }
    }

    use futures::io::{AsyncRead, AsyncWrite};



    impl AsyncRead for AllTcpStream  {

        #[project] 
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {

            #[project]
            match self.project() {
                AllTcpStream::Tcp(stream) => stream.poll_read(cx,buf),
                AllTcpStream::Tls(stream) => stream.poll_read(cx,buf)
            }

        }

    }

    impl AsyncWrite for AllTcpStream {

        #[project] 
        fn poll_write(
            self: Pin<&mut Self>, 
            cx: &mut Context, 
            buf: &[u8]
        ) -> Poll<Result<usize, io::Error>> {

            #[project]
            match self.project() {
                AllTcpStream::Tcp(stream) => stream.poll_write(cx,buf),
                AllTcpStream::Tls(stream) => stream.poll_write(cx,buf)
            }
        }

        #[project]
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {

            #[project]
            match self.project() {
                AllTcpStream::Tcp(stream) => stream.poll_flush(cx),
                AllTcpStream::Tls(stream) => stream.poll_flush(cx)
            }
        }

        #[project]
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {

            #[project]
            match self.project() {
                AllTcpStream::Tcp(stream) => stream.poll_close(cx),
                AllTcpStream::Tls(stream) => stream.poll_close(cx)
            }
        }
    }
}








#[cfg(test)]
mod test {

    use std::io::Error as IoError;
    use std::net::SocketAddr;
    use std::time;


    use log::debug;
    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use bytes::buf::ext::BufExt;
    use futures::sink::SinkExt;
    use futures::stream::StreamExt;
    use futures::future::join;
    use futures_codec::BytesCodec;
    use futures_codec::Framed;
    use async_tls::TlsConnector;
    use async_tls::TlsAcceptor;


    use crate::test_async;
    use crate::timer::sleep;

    use crate::net::TcpListener;
    use crate::net::TcpStream;

    use super::ConnectorBuilder;
    use super::AcceptorBuilder;
    use super::AllTcpStream;

    const CA_PATH: &'static str = "certs/certs/ca.crt";

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }


    #[test_async]
    async fn test_async_tls() -> Result<(), IoError> {

        
        test_tls(
            AcceptorBuilder::new_no_client_authentication()
                .load_server_certs("certs/certs/server.crt","certs/certs/server.key")?
                .build(),
            ConnectorBuilder::new()
                .no_cert_verification()
                .build()
        ).await.expect("no client cert test failed");
        
        
        // test client authentication
        
        test_tls(
            AcceptorBuilder::new_client_authenticate(CA_PATH)?
                .load_server_certs("certs/certs/server.crt","certs/certs/server.key")?
                .build(),
            ConnectorBuilder::new()
                .load_client_certs("certs/certs/client.crt","certs/certs/client.key")?
                .load_ca_cert(CA_PATH)?
                .build()
        ).await.expect("client cert test fail");
        

        Ok(())

    }
     
    async fn test_tls(acceptor: TlsAcceptor,connector: TlsConnector) -> Result<(), IoError> {
    
        let addr = "127.0.0.1:9998".parse::<SocketAddr>().expect("parse");

        let server_ft = async {
            
                debug!("server: binding");
                let listener = TcpListener::bind(&addr).await.expect("listener failed");
                debug!("server: successfully binding. waiting for incoming");
                
                let mut incoming = listener.incoming();
                while let Some(stream) = incoming.next().await {

                    let acceptor = acceptor.clone();
                    debug!("server: got connection from client");
                    let tcp_stream = stream.expect("no stream");

                    debug!("server: try to accept tls connection");
                    let handshake = acceptor.accept(tcp_stream);

                    debug!("server: handshaking");
                    let tls_stream = handshake.await.expect("hand shake failed");
                    
                    // handle connection
                    let mut framed = Framed::new(tls_stream,BytesCodec{});
                    debug!("server: sending values to client");
                    let data = vec![0x05, 0x0a, 0x63];
                    framed.send(to_bytes(data)).await.expect("send failed");
                    sleep(time::Duration::from_micros(1)).await;
                    debug!("server: sending 2nd value to client");
                    let data2 = vec![0x20,0x11]; 
                    framed.send(to_bytes(data2)).await.expect("2nd send failed");
                    return Ok(()) as Result<(),IoError>

            }
            
            Ok(()) as Result<(), IoError>
        };

        let client_ft = async {
            
            debug!("client: sleep to give server chance to come up");
            sleep(time::Duration::from_millis(100)).await;
            debug!("client: trying to connect");
            let tcp_stream = TcpStream::connect(&addr).await.expect("connection fail");
            let tls_stream = connector.connect("localhost", tcp_stream).await.expect("tls failed");
            let all_stream = AllTcpStream::Tls(tls_stream);
            let mut framed = Framed::new(all_stream,BytesCodec{});
            debug!("client: got connection. waiting");
            if let Some(value) = framed.next().await {
                debug!("client :received first value from server");
                let bytes = value.expect("invalid value");
                let values = bytes.take(3).into_inner();
                assert_eq!(values[0],0x05);
                assert_eq!(values[1],0x0a);
                assert_eq!(values[2],0x63);
                assert_eq!(values.len(),3);
            } else {
                assert!(false,"no value received");
            }

            if let Some(value) = framed.next().await {
                debug!("client: received 2nd value from server");
                let bytes = value.expect("packet decoding works");
                let values = bytes.take(2).into_inner();
                assert_eq!(values.len(),2);

            } else {
                assert!(false,"no value received");
            }

            
            Ok(()) as Result<(), IoError>
        };


        let _rt = join(client_ft,server_ft).await;

        Ok(())
    }
}