use codspeed_criterion_compat::{criterion_group, criterion_main, Criterion};
use oxhttp::model::{Body, Request, Response, Uri};
use oxhttp::{Client, Server};
use std::io;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};

fn client_server_no_body(c: &mut Criterion) {
    Server::new(|_| Response::builder().body(Body::empty()).unwrap())
        .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3456))
        .spawn()
        .unwrap();

    let client = Client::new();
    let uri = Uri::try_from("http://localhost:3456").unwrap();

    c.bench_function("client_server_no_body", |b| {
        b.iter(|| {
            client
                .request(Request::builder().uri(uri.clone()).body(()).unwrap())
                .unwrap();
        })
    });
}

fn client_server_fixed_body(c: &mut Criterion) {
    Server::new(|request| {
        let mut body = Vec::new();
        request.body_mut().read_to_end(&mut body).unwrap();
        Response::builder().body(Body::from(body)).unwrap()
    })
    .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3457))
    .spawn()
    .unwrap();

    let client = Client::new();
    let uri = Uri::try_from("http://localhost:3456").unwrap();
    let body = vec![16u8; 1024];

    c.bench_function("client_server_fixed_body", |b| {
        b.iter(|| {
            client
                .request(
                    Request::builder()
                        .uri(uri.clone())
                        .body(body.clone())
                        .unwrap(),
                )
                .unwrap();
        })
    });
}

fn client_server_chunked_body(c: &mut Criterion) {
    Server::new(|request| {
        let mut body = Vec::new();
        request.body_mut().read_to_end(&mut body).unwrap();
        Response::builder().body(Body::empty()).unwrap()
    })
    .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3458))
    .spawn()
    .unwrap();

    let client = Client::new();
    let uri = Uri::try_from("http://localhost:3456").unwrap();

    c.bench_function("client_server_chunked_body", |b| {
        b.iter(|| {
            client
                .request(
                    Request::builder()
                        .uri(uri.clone())
                        .body(Body::from_read(ChunkedReader::default()))
                        .unwrap(),
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
