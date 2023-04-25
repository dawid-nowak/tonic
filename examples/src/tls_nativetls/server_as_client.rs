pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

use hyper::server::conn::Http;
use tokio::net::TcpListener;
use pb::{echo_client::EchoClient, EchoRequest};

use tokio_native_tls::native_tls;
use std::fs::File;
use std::io::Read;

use tower::service_fn;
use tonic::transport::{Endpoint, Uri};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::pin::Pin;
use tonic::transport::Channel;

use core::task::Context;
use core::task::Poll;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;
use futures::{future};
use std::io;
use tokio::io::ReadBuf;
use crate::io::Error;
use futures::stream::Stream;
use tokio_stream::StreamExt;
use tokio::join;


#[derive(Clone)]
struct MyConnector{
    inner: Arc<Mutex<TlsStream<TcpStream>>>
}

impl tokio::io::AsyncRead for MyConnector {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>
    ) -> Poll<Result<(), Error>>{
	let maybe_lock = self.inner.try_lock();
	if let Ok(lock) = maybe_lock{
	    let mut stream = lock;
	    Pin::new(&mut (*stream)).poll_read(cx,buf)
	}else{
	    Poll::Pending
	}		
    }
}

impl tokio::io::AsyncWrite for MyConnector {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8]
    ) -> Poll<Result<usize, Error>>{
	let maybe_lock = self.inner.try_lock();
	if let Ok(lock) = maybe_lock{
	    let mut stream = lock;
	    Pin::new(&mut *stream).poll_write(cx,buf)
	}else{
	    Poll::Pending
	}	    	

    }
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Result<(), Error>>{

	let maybe_lock = self.inner.try_lock();
	if let Ok(lock) = maybe_lock{
	    let mut stream = lock;
	    Pin::new(&mut (*stream)).poll_flush(cx)
	}else{
	    Poll::Pending
	}		

	
	// let mut stream = self.inner.blocking_lock();
	// Pin::new(&mut (*stream)).poll_flush(cx)
//	Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Result<(), Error>>{

	let maybe_lock = self.inner.try_lock();
	if let Ok(lock) = maybe_lock{
	    let mut stream = lock;
	    Pin::new(&mut (*stream)).poll_shutdown(cx)
	}else{
	    Poll::Pending
	}

	
	// let mut stream = self.inner.blocking_lock();
	// Pin::new(&mut (*stream)).poll_shutdown(cx)
//	Pin::new(&mut self.inner).poll_shutdown(cx)
    }
    
}

fn echo_requests_iter() -> impl Stream<Item = EchoRequest> {
    tokio_stream::iter(1..usize::MAX).map(|i| EchoRequest {
        message: format!("msg {:02}", i),
    })
}

async fn bidirectional_streaming_echo( client:  &mut EchoClient<Channel>, num: usize) {
    tracing::info!(message = "bidirectional_streaming_echo" );
    let in_stream = echo_requests_iter().take(num);

    let response = client
        .bidirectional_streaming_echo(in_stream)
        .await
        .unwrap();

    let mut resp_stream = response.into_inner();

    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
	let msg = format!("received message {:#?}", received.message);
	tracing::info!(message = "Got message ", %msg);
        
    }
    tracing::info!(message = "bidirectional_streaming_echo done");
}


//#[tokio::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    let data_dir = std::path::PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "data"]);
    

    let mut file = File::open(data_dir.join("tls/server.key")).unwrap();
    let mut key = vec![];
    file.read_to_end(&mut key).unwrap();

    let mut file = File::open(data_dir.join("tls/server.pem")).unwrap();
    let mut certs = vec![];
    file.read_to_end(&mut certs).unwrap();
    
    let identity = native_tls::Identity::from_pkcs8(&certs,&key)?;
    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(identity).build()?);
    
        
    //let listener = TcpListener::bind("[::1]:50051").await?;
    let listener = rt.block_on(TcpListener::bind("[::1]:50051"))?;
    let http = Http::new();    
    
    
    loop {
        // Asynchronously wait for an inbound socket.
	//let (socket, remote_addr) = listener.accept().await?;
        let (socket, remote_addr) = rt.block_on(listener.accept())?;
        let tls_acceptor = tls_acceptor.clone();	
	let mut http = http.clone();
	http.http2_only(true);


	tracing::info!(message = "Accept connection from .", %remote_addr);
        
	
        rt.spawn(async move  {
            // Accept the TLS connection.
	    let tls_stream = tls_acceptor.accept(socket).await.expect("accept error");

	    let connector = MyConnector{
		inner: Arc::new(Mutex::new(tls_stream))
	    };


	    let channel = Endpoint::try_from("http://[::1]:50051").unwrap()
		.connect_with_connector(service_fn(move |_: Uri| {
		    let connector = connector.clone();
		    
		    future::ready(Ok::<MyConnector, io::Error>(connector))
		})).await.unwrap();
	    
	    let client = EchoClient::new(channel);
	    let client = Arc::new(Mutex::new(client));
	    let unary_client =  client.clone();
	    let stream_client =  client.clone();
	    
	    let unary_task = tokio::spawn( async move  {		
		let request = tonic::Request::new(EchoRequest {
		    message: "hello".into(),
		});
		let client = unary_client;
		let mut client = client.lock().await;
		tracing::info!(message = format!("Unary request {:?}", request));
		let response = client.unary_echo(request).await.unwrap();
		tracing::info!(message = format!("Unary response {:?}", response));
	    });

	    let stream_task = tokio::spawn( async move  {
		let client = stream_client;
		let mut client = client.lock().await;
		bidirectional_streaming_echo(&mut *client, 100).await;
	    });

	    let res = join!(stream_task,unary_task);
	    tracing::info!(message = format!("gRPC client done {:?}", res));
        });
	
    }    
}


