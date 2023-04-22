//! This examples shows how you can combine `hyper-rustls` and `tonic` to
//! provide a custom `ClientConfig` for the tls configuration.

pub mod pb {
    tonic::include_proto!("/grpc.examples.unaryecho");
}

use std::iter::FromIterator;
use pb::{EchoRequest,EchoResponse};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use std::io::Read;
use tokio::net::TcpStream;
use tonic::{transport::Server, Request, Response, Status};
use hyper::server::conn::Http;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = std::path::PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "data"]);
    let mut fd = std::fs::File::open(data_dir.join("tls/ca.pem"))?;

    let mut root = vec![];
    fd.read_to_end(&mut root).unwrap();
    let roots = native_tls::Certificate::from_pem(&root).unwrap();
            

    let mut http = Http::new();
    http.http2_only(true);	

    let tls_connector = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::builder()
							     .add_root_certificate(roots)
							     .danger_accept_invalid_certs(true)
							     .request_alpns(&["h2"]).build().unwrap());
    

    let stream = TcpStream::connect("[::1]:50051").await?;
    let tls_stream = tls_connector.connect("http://[::1]:50051",stream).await?;

    let server = EchoServer::default();
    let svc = Server::builder()
        .add_service(pb::echo_server::EchoServer::new(server))
        .into_service();
    http.serve_connection(tls_stream, svc).await.unwrap();            
    Ok(())
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
