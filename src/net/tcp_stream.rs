


use std::io::Error;
use std::net::SocketAddr;
use std::time;

use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use bytes::buf::ext::BufExt;
use futures_util::sink::SinkExt;
use futures_lite::stream::StreamExt;
use futures_lite::future::zip;

use tokio_util::codec::BytesCodec;
use tokio_util::codec::Framed;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::log::debug;

use crate::test_async;
use crate::timer::sleep;

use crate::net::TcpListener;
use crate::net::TcpStream;


fn to_bytes(bytes: Vec<u8>) -> Bytes {
    let mut buf = BytesMut::with_capacity(bytes.len());
    buf.put_slice(&bytes);
    buf.freeze()
}




#[test_async]
async fn test_async_tcp() -> Result<(), Error> {
    let addr = "127.0.0.1:9998".parse::<SocketAddr>().expect("parse");

    let server_ft = async {
        
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await?;
            debug!("server: successfully binding. waiting for incoming");
            let mut incoming = listener.incoming();
            while let Some(stream) = incoming.next().await {
                debug!("server: got connection from client");
                let tcp_stream = stream?;
                let mut framed = Framed::new(tcp_stream.compat(),BytesCodec::new());
                debug!("server: sending values to client");
                let data = vec![0x05, 0x0a, 0x63];
                framed.send(to_bytes(data)).await?;
                return Ok(()) as Result<(),Error>

        }
        
        Ok(()) as Result<(), Error>
    };

    let client_ft = async {
        
        debug!("client: sleep to give server chance to come up");
        sleep(time::Duration::from_millis(100)).await;
        debug!("client: trying to connect");
        let tcp_stream = TcpStream::connect(&addr).await?;
        let mut framed = Framed::new(tcp_stream.compat(),BytesCodec::new());
        debug!("client: got connection. waiting");
        if let Some(value) = framed.next().await {
            debug!("client :received first value from server");
            let bytes = value?;
            debug!("client :received bytes len: {}",bytes.len());
            assert_eq!(bytes.len(),3);
            let values = bytes.take(3).into_inner();
            assert_eq!(values[0],0x05);
            assert_eq!(values[1],0x0a);
            assert_eq!(values[2],0x63);
        } else {
            assert!(false,"no value received");
        }


        
        Ok(()) as Result<(), Error>
    };


    let _ = zip(client_ft,server_ft).await;

    Ok(())
}

