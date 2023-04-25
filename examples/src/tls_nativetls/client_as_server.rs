//! This examples shows how you can combine `hyper-rustls` and `tonic` to
//! provide a custom `ClientConfig` for the tls configuration.

pub mod pb {
    tonic::include_proto!("/grpc.examples.echo");
}

use futures::Stream;
use std::iter::FromIterator;
use pb::{EchoRequest,EchoResponse};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use std::io::Read;
use tokio::net::TcpStream;
use tonic::{transport::Server, Request, Response, Status,Streaming};
use hyper::server::conn::Http;
use std::{error::Error, io::ErrorKind, pin::Pin};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio::sync::mpsc;


type EchoResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;



//#[tokio::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let rt = tokio::runtime::Runtime::new().unwrap();
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
    
    
    let stream = rt.block_on(TcpStream::connect("[::1]:50051"))?;//.await
    let tls_stream = rt.block_on(tls_connector.connect("http://[::1]:50051",stream))?;//.await?;

    let server = EchoServer::default();
    let svc = Server::builder()
        .add_service(pb::echo_server::EchoServer::new(server))
        .into_service();
    rt.block_on(http.serve_connection(tls_stream, svc))?;
    Ok(())
}


#[derive(Default)]
pub struct EchoServer;

#[tonic::async_trait]
impl pb::echo_server::Echo for EchoServer {
    async fn unary_echo(&self, request: Request<EchoRequest>) -> EchoResult<EchoResponse> {
        let request = request.into_inner();
	tracing::info!(message = format!("EchoServer::unary_echo {:#?}", request));
        Ok(Response::new(EchoResponse { message: request.message }))
    }

    type ServerStreamingEchoStream = ResponseStream;
    
    async fn server_streaming_echo(
        &self,
        _req: Request<EchoRequest>,
    ) -> EchoResult<Self::ServerStreamingEchoStream> {
	Err(Status::unimplemented("not implemented"))
    }

    async fn client_streaming_echo(
        &self,
        _: Request<Streaming<EchoRequest>>,
    ) -> EchoResult<EchoResponse> {
        Err(Status::unimplemented("not implemented"))
    }

    type BidirectionalStreamingEchoStream = ResponseStream;

    async fn bidirectional_streaming_echo(
        &self,
        req: Request<Streaming<EchoRequest>>,
    ) -> EchoResult<Self::BidirectionalStreamingEchoStream> {
	tracing::info!(message = "EchoServer::bidirectional_streaming_echo");


        let mut in_stream = req.into_inner();
        let (tx, rx) = mpsc::channel(128);

        // this spawn here is required if you want to handle connection error.
        // If we just map `in_stream` and write it back as `out_stream` the `out_stream`
        // will be drooped when connection error occurs and error will never be propagated
        // to mapped version of `in_stream`.
        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(v) => tx
                        .send(Ok(EchoResponse { message: v.message }))
                        .await
                        .expect("working rx"),
                    Err(err) => {
                        if let Some(io_err) = match_for_io_error(&err) {
                            if io_err.kind() == ErrorKind::BrokenPipe {
                                // here you can handle special case when client
                                // disconnected in unexpected way
                                eprintln!("\tclient disconnected: broken pipe");
                                break;
                            }
                        }

                        match tx.send(Err(err)).await {
                            Ok(_) => (),
                            Err(_err) => break, // response was droped
                        }
                    }
                }
            }
            println!("\tstream ended");
        });

        // echo just write the same data that was received
        let out_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(out_stream) as Self::BidirectionalStreamingEchoStream
        ))
    }
}

fn match_for_io_error(err_status: &Status) -> Option<&std::io::Error> {
    let mut err: &(dyn Error + 'static) = err_status;

    loop {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return Some(io_err);
        }

        // h2::Error do not expose std::io::Error with `source()`
        // https://github.com/hyperium/h2/pull/462
        if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
            if let Some(io_err) = h2_err.get_io() {
                return Some(io_err);
            }
        }

        err = match err.source() {
            Some(err) => err,
            None => return None,
        };
    }
}
