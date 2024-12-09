use codspeed_criterion_compat::{criterion_group, criterion_main, Criterion};
use oxhttp::model::{Body, Method, Request, Response, Status};
use oxhttp::{Client, Server};
use std::io;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};
use url::Url;

fn client_server_no_body(c: &mut Criterion) {
    Server::new(|_| Response::builder(Status::OK).build())
        .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3456))
        .spawn()
        .unwrap();

    let client = Client::new();
    let url = Url::parse("http://localhost:3456").unwrap();

    c.bench_function("client_server_no_body", |b| {
        b.iter(|| {
            client
                .request(Request::builder(Method::GET, url.clone()).build())
                .unwrap();
        })
    });
}

fn client_server_fixed_body(c: &mut Criterion) {
    Server::new(|request| {
        let mut body = Vec::new();
        request.body_mut().read_to_end(&mut body).unwrap();
        Response::builder(Status::OK).with_body(body)
    })
    .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3457))
    .spawn()
    .unwrap();

    let client = Client::new();
    let url = Url::parse("http://localhost:3456").unwrap();
    let body = vec![16u8; 1024];

    c.bench_function("client_server_fixed_body", |b| {
        b.iter(|| {
            client
                .request(Request::builder(Method::GET, url.clone()).with_body(body.clone()))
                .unwrap();
        })
    });
}

fn client_server_chunked_body(c: &mut Criterion) {
    Server::new(|request| {
        let mut body = Vec::new();
        request.body_mut().read_to_end(&mut body).unwrap();
        Response::builder(Status::OK).build()
    })
    .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3458))
    .spawn()
    .unwrap();

    let client = Client::new();
    let url = Url::parse("http://localhost:3456").unwrap();

    c.bench_function("client_server_chunked_body", |b| {
        b.iter(|| {
            client
                .request(
                    Request::builder(Method::GET, url.clone())
                        .with_body(Body::from_read(ChunkedReader::default())),
                )
                .unwrap();
        })
    });
}

criterion_group!(
    client_server,
    client_server_no_body,
    client_server_fixed_body,
    client_server_chunked_body
);

criterion_main!(client_server);

#[derive(Default)]
struct ChunkedReader {
    counter: usize,
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.counter >= 10 {
            return Ok(0);
        }
        self.counter += 1;
        buf.fill(12);
        Ok(buf.len())
    }
}
