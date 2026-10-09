use std::{
    io::{Read, Write},
    net::{Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    sync::Arc,
    time::Duration,
};
use tos_access::{
    AccessError, AccessExecutor, AccessProfile, DisclosureFence, PreparedHealth, PreparedPacket,
    http::serve_connection,
};
use tos_query::AbortProbe;

struct Fence;
impl DisclosureFence for Fence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        Ok(())
    }
}

struct HealthOwner {
    body: Vec<u8>,
    ready: bool,
}
impl AccessExecutor for HealthOwner {
    fn source_descend_available(&self) -> bool {
        false
    }

    fn source_descend(
        &self,
        _: tos_access::Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        unreachable!("health-only test owner has no source descent capability")
    }

    fn access_health_available(&self) -> bool {
        true
    }

    fn access_health_report(
        &self,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedHealth<'static>, AccessError> {
        Ok(PreparedHealth {
            packet: PreparedPacket {
                body: self.body.clone(),
                fence: Box::new(Fence),
            },
            ok: self.ready,
        })
    }
}

fn profile() -> AccessProfile {
    AccessProfile::new(65_536, 1_048_576, 65_536).with_query_timeout(Duration::from_secs(5))
}

fn roundtrip(address: SocketAddr, method: &str) -> (u16, usize, Vec<u8>) {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "{method} /health HTTP/1.1\r\nHost: [::1]\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let headers = std::str::from_utf8(&response[..split]).unwrap();
    let status = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    (status, content_length, response[split + 4..].to_vec())
}

fn ipv6_listener() -> Option<(TcpListener, SocketAddr)> {
    match TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
        Ok(listener) => {
            let address = listener.local_addr().unwrap();
            assert!(address.is_ipv6());
            assert_eq!(address.ip(), Ipv6Addr::LOCALHOST);
            Some((listener, address))
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
            ) =>
        {
            eprintln!("IPv6 loopback unavailable: {error}");
            None
        }
        Err(error) => panic!("cannot bind IPv6 loopback: {error}"),
    }
}

#[test]
fn native_health_http_roundtrips_on_ipv6_and_preserves_owner_readiness() {
    let Some((listener, address)) = ipv6_listener() else {
        return;
    };
    let healthy_body =
        br#"{"schema_version":"tos_selected_access_health_v1","ok":true,"errors":[]}"#.to_vec();
    let unhealthy_body = br#"{"schema_version":"tos_selected_access_health_v1","ok":false,"errors":["projection invalid"]}"#.to_vec();
    let healthy_length = healthy_body.len();
    let server = std::thread::spawn(move || {
        for (body, ready) in [
            (healthy_body.clone(), true),
            (healthy_body, true),
            (unhealthy_body, false),
        ] {
            let (stream, peer) = listener.accept().unwrap();
            assert_eq!(peer.ip(), Ipv6Addr::LOCALHOST);
            serve_connection(stream, Arc::new(HealthOwner { body, ready }), profile());
        }
    });

    let (status, length, body) = roundtrip(address, "GET");
    assert_eq!(status, 200);
    assert_eq!(length, body.len());
    assert!(
        body.windows(b"\"ok\":true".len())
            .any(|part| part == b"\"ok\":true")
    );

    let (status, length, body) = roundtrip(address, "HEAD");
    assert_eq!(status, 200);
    assert_eq!(length, healthy_length);
    assert!(body.is_empty());

    let (status, length, body) = roundtrip(address, "GET");
    assert_eq!(status, 503);
    assert_eq!(length, body.len());
    assert!(
        body.windows(b"\"ok\":false".len())
            .any(|part| part == b"\"ok\":false")
    );
    server.join().unwrap();
}
