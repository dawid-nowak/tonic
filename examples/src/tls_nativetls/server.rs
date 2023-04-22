pub mod pb {
    tonic::include_proto!("/grpc.examples.unaryecho");
}

use hyper::server::conn::Http;
use pb::{EchoRequest, EchoResponse};
use tokio::net::TcpListener;

use tokio_native_tls::native_tls;
use std::fs::File;
use std::io::Read;

use tonic::{transport::Server, Request, Response, Status};

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
    let tls_acceptor =  tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(identity).build()?);
   

    
    let listener = TcpListener::bind("[::1]:50051").await?;
    let server = EchoServer::default();

    let svc = Server::builder()
        .add_service(pb::echo_server::EchoServer::new(server))
        .into_service();

    let http = Http::new();
    

    // loop{
    // 	let (socket, remote_addr) = listener.accept().await?;
    // 	let mut http = http.clone();
    //  	http.http2_only(false);
    //  	http.http1_only(false);
    // 	let svc = svc.clone();
    // 	println!("accept connection from {}", remote_addr);
	
    // 	tokio::spawn(async move {
    //         http.serve_connection(socket, svc).await.unwrap();
    //     });    
	
    // }
    
    loop {
        // Asynchronously wait for an inbound socket.
        let (socket, remote_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();	
	let mut http = http.clone();
	http.http2_only(true);	
        let svc = svc.clone();
        println!("accept connection from {}", remote_addr);
	
        tokio::spawn(async move {
            // Accept the TLS connection.
            let tls_stream = tls_acceptor.accept(socket).await.expect("accept error");
            // In a loop, read data from the socket and write the data back.
            http.serve_connection(tls_stream, svc).await.unwrap();

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
