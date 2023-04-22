//! This examples shows how you can combine `hyper-rustls` and `tonic` to
//! provide a custom `ClientConfig` for the tls configuration.

pub mod pb {
    tonic::include_proto!("/grpc.examples.unaryecho");
}

use std::iter::FromIterator;

use hyper::{client::HttpConnector, Uri};
use pb::{echo_client::EchoClient, EchoRequest};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use std::io::Read;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = std::path::PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "data"]);
    let mut fd = std::fs::File::open(data_dir.join("tls/ca.pem"))?;

    let mut root = vec![];
    fd.read_to_end(&mut root).unwrap();
    let roots = native_tls::Certificate::from_pem(&root).unwrap();
            
    let mut http = HttpConnector::new();
    http.enforce_http(false);

    let tls_connector = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::builder()
		.add_root_certificate(roots)
		.request_alpns(&["h2"]).build().unwrap());
    
    let connector = hyper_tls::HttpsConnector::from((http,tls_connector));
        
    let client = hyper::Client::builder().http2_only(true).build(connector);
    
    let uri = Uri::from_static("https://example.com");
    let uri = Uri::from_static("https://[::1]:50051");

    
 
    
    let mut client = EchoClient::with_origin(client, uri);

    let request = tonic::Request::new(EchoRequest {
        message: "hello".into(),
    });

    let response = client.unary_echo(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}

