use codspeed_criterion_compat::{criterion_group, criterion_main, Criterion};
use oxhttp::model::{Body, Request, Response, Uri};
use oxhttp::{Client, Server};
use std::collections::HashSet;
use std::io;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Mutex, OnceLock};

fn spawn_server_once(
    on_request: impl Fn(&mut Request<Body>) -> Response<Body> + Send + Sync + 'static,
    port: u16,
) -> Uri {
    static SERVERS: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();
    let mut servers = SERVERS.get_or_init(Mutex::default).lock().unwrap();
    if !servers.contains(&port) {
        Server::new(on_request)
            .bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
            .spawn()
            .unwrap();
        servers.insert(port);
    }
    Uri::try_from(format!("http://localhost:{port}")).unwrap()
}

fn client_server_no_body(c: &mut Criterion) {
    let client = Client::new();

    c.bench_function("client_server_no_body", |b| {
        let uri = spawn_server_once(|_| Response::builder().body(Body::empty()).unwrap(), 3456);
        b.iter(|| {
            client
                .request(Request::builder().uri(uri.clone()).body(()).unwrap())
                .unwrap();
        })
    });
}

fn client_server_fixed_body(c: &mut Criterion) {
    let client = Client::new();
    let body = vec![16u8; 1024];

    c.bench_function("client_server_fixed_body", |b| {
        let uri = spawn_server_once(
            |request| {
                let mut body = Vec::new();
                request.body_mut().read_to_end(&mut body).unwrap();
                Response::builder().body(Body::from(body)).unwrap()
            },
            3457,
        );
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
    c.bench_function("client_server_chunked_body", |b| {
        let uri = spawn_server_once(
            |request| {
                let mut body = Vec::new();
                request.body_mut().read_to_end(&mut body).unwrap();
                Response::builder().body(Body::empty()).unwrap()
            },
            3458,
        );
        let client = Client::new();
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
