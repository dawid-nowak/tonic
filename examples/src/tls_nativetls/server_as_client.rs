pub mod pb {
    tonic::include_proto!("/grpc.examples.unaryecho");
}

use hyper::server::conn::Http;
use tokio::net::TcpListener;
use pb::{echo_client::EchoClient, EchoRequest,EchoResponse};

use tokio_native_tls::native_tls;
use std::fs::File;
use std::io::Read;

use tonic::{Request, Response, Status};
use tower::service_fn;
use tonic::transport::{Endpoint, Uri};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::pin::Pin;

use core::task::Context;
use core::task::Poll;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;
use futures::{future};
use std::io;
use tokio::io::ReadBuf;
use crate::io::Error;


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


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = std::path::PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "data"]);
    

    let mut file = File::open(data_dir.join("tls/server.key")).unwrap();
    let mut key = vec![];
    file.read_to_end(&mut key).unwrap();

    let mut file = File::open(data_dir.join("tls/server.pem")).unwrap();
    let mut certs = vec![];
    file.read_to_end(&mut certs).unwrap();
    
    let identity = native_tls::Identity::from_pkcs8(&certs,&key)?;
    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(identity).build()?);
    
        
    let listener = TcpListener::bind("[::1]:50051").await?;
    let http = Http::new();    
    
    
    loop {
        // Asynchronously wait for an inbound socket.
        let (socket, remote_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();	
	let mut http = http.clone();
	http.http2_only(true);
			    
        println!("accept connection from {}", remote_addr);
	
        tokio::spawn(async move  {
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
	                	    
	    let mut client = EchoClient::new(channel);
	    let request = tonic::Request::new(EchoRequest {
		message: "hello".into(),
	    });

	    let response = client.unary_echo(request).await.unwrap();
	    println!("RESPONSE={:?}", response);	    
        });    
    }    
}

type EchoResult<T> = Result<Response<T>, Status>;

#[derive(Default)]
pub struct EchoServer;

#[tonic::async_trait]
impl pb::echo_server::Echo for EchoServer {
    async fn unary_echo(&self, request: Request<EchoRequest>) -> EchoResult<EchoResponse> {
        
        println!(
            "Got a request!"
        );

        let message = request.into_inner().message;
        Ok(Response::new(EchoResponse { message }))
    }
}
