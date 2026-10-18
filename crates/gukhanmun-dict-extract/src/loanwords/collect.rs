// Gukhanmun: Experimental source-independent loanword extraction tooling.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Bounded HTTPS collector, isolated from default builds.
//! The response shape is live-verified; configured limits are local safeguards.
use super::{ENDPOINT, Error, Page, Snapshot};
use std::{io::Read, thread, time::Duration};

/// Hard-bounded collection settings; validate before making any request.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Requested rows per page, between 1 and 1000.
    pub rows: u64,
    /// Maximum pages per sweep, between 1 and 10000.
    pub max_pages: u64,
    /// Maximum uncompressed response bytes per page, at most 16 MiB.
    pub max_body_bytes: u64,
    /// Maximum accepted response bytes across both sweeps, at most 256 MiB.
    /// Failed reads also count; one extra byte may be read to detect overflow.
    pub max_total_bytes: u64,
    /// Request timeout, between one millisecond and 60 seconds.
    pub timeout: Duration,
    /// Retries after transient failure, at most three.
    pub retries: u32,
    /// Minimum delay before each request, at most 60 seconds.
    pub delay: Duration,
    /// Maximum accepted numeric Retry-After delay, at most 60 seconds.
    pub max_retry_after: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            rows: 100,
            max_pages: 1000,
            max_body_bytes: 4 * 1024 * 1024,
            max_total_bytes: 128 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            retries: 2,
            delay: Duration::from_secs(1),
            max_retry_after: Duration::from_secs(60),
        }
    }
}
impl Limits {
    /// Rejects zero/unbounded settings before a credential-bearing request exists.
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=1000).contains(&self.rows)
            || !(1..=10000).contains(&self.max_pages)
            || !(1..=16 * 1024 * 1024).contains(&self.max_body_bytes)
            || !(1..=256 * 1024 * 1024).contains(&self.max_total_bytes)
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(60)
            || self.retries > 3
            || self.delay > Duration::from_secs(60)
            || self.max_retry_after > Duration::from_secs(60)
        {
            return Err(Error::Invalid("collection limits"));
        }
        Ok(())
    }
}

// Accept either a decoded key or a percent-encoded key, decoding exactly once.
// A literal plus is credential data, never an application/x-www-form-urlencoded space.
fn decoded_key(key: &str) -> Result<String, Error> {
    let invalid = || Error::Invalid("service key encoding");
    let mut bytes = Vec::with_capacity(key.len());
    let mut chars = key.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        bytes.push(if byte == b'%' {
            let high = char::from(chars.next().ok_or_else(invalid)?)
                .to_digit(16)
                .ok_or_else(invalid)?;
            let low = char::from(chars.next().ok_or_else(invalid)?)
                .to_digit(16)
                .ok_or_else(invalid)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    let key = String::from_utf8(bytes).map_err(|_| invalid())?;
    if key.is_empty() || key.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(key)
}

fn request(
    agent: &ureq::Agent,
    endpoint: &str,
    key: &str,
    page: u64,
    limits: &Limits,
    remaining_bytes: &mut u64,
) -> Result<Vec<u8>, Error> {
    for attempt in 0..=limits.retries {
        if *remaining_bytes == 0 {
            return Err(Error::Invalid("collection byte limit"));
        }
        thread::sleep(limits.delay);
        let result = agent
            .get(endpoint)
            .header("User-Agent", "Gukhanmun loanword collector")
            .header("Accept", "application/json")
            .query("serviceKey", key)
            .query("pageNo", page.to_string())
            .query("numOfRows", limits.rows.to_string())
            .query("langType", "0003")
            .query("resultType", "json")
            .call();
        match result {
            Err(_) => {
                if attempt == limits.retries {
                    return Err(Error::Transport);
                }
            }
            Ok(mut response) => {
                let status = response.status().as_u16();
                if status == 200 {
                    let limit = limits.max_body_bytes.min(*remaining_bytes);
                    let mut bytes = Vec::new();
                    let result = response
                        .body_mut()
                        .with_config()
                        // One lookahead byte distinguishes exact EOF from overflow.
                        .limit(limit + 1)
                        .reader()
                        .read_to_end(&mut bytes)
                        .map_err(ureq::Error::from);
                    // Partial reads consume the same shared budget as successful reads.
                    *remaining_bytes = remaining_bytes.saturating_sub(bytes.len() as u64);
                    if bytes.len() as u64 > limit {
                        return Err(Error::Invalid(if limit < limits.max_body_bytes {
                            "collection byte limit"
                        } else {
                            "response body byte limit"
                        }));
                    }
                    match result {
                        Ok(_) => return Ok(bytes),
                        Err(ureq::Error::BodyExceedsLimit(_)) => {
                            if limit < limits.max_body_bytes {
                                return Err(Error::Invalid("collection byte limit"));
                            }
                            return Err(Error::Invalid("response body byte limit"));
                        }
                        Err(_) if attempt < limits.retries => continue,
                        Err(_) => return Err(Error::Transport),
                    }
                }
                if (status == 429 || (500..=599).contains(&status)) && attempt < limits.retries {
                    let delay = response
                        .headers()
                        .get("retry-after")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or(limits.delay)
                        .min(limits.max_retry_after);
                    thread::sleep(delay);
                } else {
                    return Err(Error::Http(status));
                }
            }
        }
    }
    Err(Error::Transport)
}
fn run(
    endpoint: &str,
    key: &str,
    limits: &Limits,
    collected_at: String,
) -> Result<Snapshot, Error> {
    limits.validate()?;
    if log::STATIC_MAX_LEVEL > log::LevelFilter::Debug {
        return Err(Error::Invalid(
            "credential-bearing transport requires trace logging compiled out",
        ));
    }
    let key = decoded_key(key)?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(limits.timeout))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut sweeps = Vec::new();
    let mut remaining_bytes = limits.max_total_bytes;
    for _ in 0..2 {
        let mut pages = Vec::new();
        let mut expected = 1;
        for number in 1..=limits.max_pages {
            let body = request(&agent, endpoint, &key, number, limits, &mut remaining_bytes)?;
            let page = Page::parse(&body, number, limits.rows)?;
            if number == 1 {
                expected = page.total.div_ceil(limits.rows).max(1);
                if expected > limits.max_pages {
                    return Err(Error::Invalid("collection page limit"));
                }
            }
            pages.push(page);
            if number == expected {
                break;
            }
        }
        sweeps.push(Snapshot::from_pages(pages, collected_at.clone())?);
    }
    if sweeps[0].records != sweeps[1].records {
        return Err(Error::Invalid("full collection sweeps differ"));
    }
    Ok(sweeps.remove(1))
}
/// Collects two complete matching sweeps from the fixed official HTTPS endpoint.
/// Reads `NIKL_LOANWORD_API_KEY` from the environment, with deprecated
/// `KOREAN_GO_KR_SERVICE_KEY` fallback; conflicting simultaneous values fail. Accepts decoded or
/// percent-encoded keys; decodes percent escapes exactly once and preserves pluses.
/// Errors have no raw
/// client source chain. Matching sweeps detect drift, not server-pinned generations.
/// The caller must separately establish permission to collect and reuse data.
pub fn collect(limits: &Limits, collected_at: String) -> Result<Snapshot, Error> {
    let current = std::env::var("NIKL_LOANWORD_API_KEY").ok();
    let legacy = std::env::var("KOREAN_GO_KR_SERVICE_KEY").ok();
    if current
        .as_ref()
        .zip(legacy.as_ref())
        .is_some_and(|(a, b)| a != b)
    {
        return Err(Error::Invalid(
            "conflicting service key environment variables",
        ));
    }
    let key = current
        .or(legacy)
        .ok_or(Error::Invalid("missing service key environment variable"))?;
    run(ENDPOINT, &key, limits, collected_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{self, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Instant,
    };
    fn accept_with_timeout(listener: &TcpListener, timeout: Duration) -> io::Result<TcpStream> {
        listener.set_nonblocking(true)?;
        let start = Instant::now();
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    // Accepted sockets do not inherit blocking mode consistently across OSes.
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(timeout))?;
                    stream.set_write_timeout(Some(timeout))?;
                    return Ok(stream);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    if start.elapsed() >= timeout {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "mock server did not receive a connection",
                        ));
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn accept(listener: &TcpListener) -> TcpStream {
        accept_with_timeout(listener, Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn mock_server_waits_are_bounded_without_a_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = thread::spawn(move || {
            accept_with_timeout(&listener, Duration::from_millis(50)).unwrap_err()
        });
        assert_eq!(server.join().unwrap().kind(), io::ErrorKind::TimedOut);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _silent_client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut stream = accept_with_timeout(&listener, Duration::from_millis(50)).unwrap();
        let error = stream.read(&mut [0; 1]).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
    }
    fn page() -> String {
        json!({"resultCode":"00","resultMsg":"OK","pageNo":1,"numOfRows":1,"totalCount":1,"items":[{"example_no":"synthetic-1","srclang_mark":"香港","korean_mark":"홍콩","regltn_code":"0003"}]}).to_string()
    }
    fn mock(
        replies: Vec<(u16, String, String)>,
    ) -> (String, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        mock_key(replies, "sentinel-secret")
    }
    fn mock_key(
        replies: Vec<(u16, String, String)>,
        expected_key: &'static str,
    ) -> (String, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let handle = thread::spawn(move || {
            for (status, headers, body) in replies {
                let mut stream = accept(&listener);
                let mut request = [0; 8192];
                let n = stream.read(&mut request).unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..n])
                        .contains(&format!("serviceKey={expected_key}&"))
                );
                count.fetch_add(1, Ordering::SeqCst);
                write!(stream,"HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",body.len()).unwrap();
            }
        });
        (url, hits, handle)
    }
    fn limits() -> Limits {
        Limits {
            rows: 1,
            delay: Duration::ZERO,
            max_retry_after: Duration::ZERO,
            // Ordinary requests need scheduling headroom on loaded CI runners.
            timeout: Duration::from_secs(5),
            ..Limits::default()
        }
    }
    #[test]
    fn service_keys_are_encoded_once_and_cannot_inject_query_parameters() {
        for key in ["sentinel+/=secret", "sentinel%2B%2f%3Dsecret"] {
            let (url, hits, h) = mock_key(
                vec![(200, String::new(), page()); 2],
                "sentinel%2B%2F%3Dsecret",
            );
            run(&url, key, &limits(), "synthetic".into()).unwrap();
            h.join().unwrap();
            assert_eq!(hits.load(Ordering::SeqCst), 2);
        }
        let (url, _, h) = mock_key(
            vec![(200, String::new(), page()); 2],
            "sentinel%26pageNo%3D99",
        );
        run(
            &url,
            "sentinel%26pageNo%3D99",
            &limits(),
            "synthetic".into(),
        )
        .unwrap();
        h.join().unwrap();
        for key in [
            "",
            "secret%",
            "secret%2",
            "secret%GG",
            "secret%FF",
            "secret%0A",
        ] {
            let error = run("http://127.0.0.1:1", key, &limits(), "synthetic".into()).unwrap_err();
            assert!(matches!(error, Error::Invalid("service key encoding")));
            assert!(!format!("{error:?} {error}").contains("secret"));
        }
        assert_eq!(decoded_key("%252B").unwrap(), "%2B");
    }
    #[test]
    fn successful_headers_with_truncated_body_are_retried() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let h = thread::spawn(move || {
            for body in [None, Some(page()), Some(page())] {
                let mut stream = accept(&listener);
                let mut request = [0; 8192];
                let _ = stream.read(&mut request).unwrap();
                if let Some(body) = body {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                } else {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nConnection: close\r\n\r\n{",
                        )
                        .unwrap();
                }
            }
        });
        run(&url, "sentinel-secret", &limits(), "synthetic".into()).unwrap();
        h.join().unwrap();
    }
    #[test]
    fn truncated_reads_consume_the_shared_byte_budget() {
        for exhausted_on_retry in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let body = page();
            let size = body.len() as u64;
            let h = thread::spawn(move || {
                let requests = if exhausted_on_retry { 1 } else { 3 };
                for number in 0..requests {
                    let mut stream = accept(&listener);
                    let mut request = [0; 8192];
                    assert!(stream.read(&mut request).unwrap() > 0);
                    let declared = if number == 0 {
                        body.len() + 100
                    } else {
                        body.len()
                    };
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n{body}").unwrap();
                }
            });
            let mut limits = limits();
            limits.max_total_bytes = if exhausted_on_retry {
                size
            } else {
                3 * size - 1
            };
            assert!(matches!(
                run(&url, "sentinel-secret", &limits, "synthetic".into()),
                Err(Error::Invalid("collection byte limit"))
            ));
            h.join().unwrap();
        }
    }
    #[test]
    fn exact_total_byte_limit_succeeds() {
        let (url, _, h) = mock(vec![(200, String::new(), page()); 2]);
        let mut limits = limits();
        limits.max_total_bytes = 2 * page().len() as u64;
        run(&url, "sentinel-secret", &limits, "synthetic".into()).unwrap();
        h.join().unwrap();
    }
    #[test]
    fn two_sweeps_and_retry() {
        let (url, hits, h) = mock(vec![
            (429, "Retry-After: 999999\r\n".into(), "secret body".into()),
            (200, String::new(), page()),
            (200, String::new(), page()),
        ]);
        assert_eq!(
            run(&url, "sentinel-secret", &limits(), "synthetic".into())
                .unwrap()
                .total_count,
            1
        );
        h.join().unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }
    #[test]
    fn redirects_auth_and_error_bodies_are_redacted() {
        for status in [302, 401, 403, 500] {
            let (url, hits, h) = mock(vec![(
                status,
                "Location: http://127.0.0.1:1/?serviceKey=sentinel-secret\r\n".into(),
                "sentinel-secret".into(),
            )]);
            let mut l = limits();
            l.retries = 0;
            let error = run(&url, "sentinel-secret", &l, "synthetic".into()).unwrap_err();
            h.join().unwrap();
            assert!(!format!("{error:?} {error}").contains("sentinel-secret"));
            assert!(matches!(error,Error::Http(s) if s==status));
            assert_eq!(hits.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn body_size_schema_and_drift_fail() {
        let (url, _, h) = mock(vec![(200, String::new(), page())]);
        let mut l = limits();
        l.max_body_bytes = 10;
        assert!(run(&url, "sentinel-secret", &l, "synthetic".into()).is_err());
        h.join().unwrap();
        let (url, _, h) = mock(vec![(200, String::new(), "sentinel-secret".into())]);
        assert!(run(&url, "sentinel-secret", &limits(), "synthetic".into()).is_err());
        h.join().unwrap();
        let changed = page().replace("홍콩", "샹강");
        let (url, _, h) = mock(vec![
            (200, String::new(), page()),
            (200, String::new(), changed),
        ]);
        assert!(run(&url, "sentinel-secret", &limits(), "synthetic".into()).is_err());
        h.join().unwrap();
    }
    #[test]
    fn retries_and_transport_are_bounded() {
        let (url, hits, h) = mock(vec![(503, String::new(), String::new()); 3]);
        assert!(matches!(
            run(&url, "sentinel-secret", &limits(), "synthetic".into()),
            Err(Error::Http(503))
        ));
        h.join().unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        assert!(matches!(
            run(
                "http://127.0.0.1:1",
                "sentinel-secret",
                &limits(),
                "synthetic".into()
            ),
            Err(Error::Transport)
        ));
    }
    #[test]
    fn trace_logger_never_sees_credentials() {
        use std::sync::Mutex;
        struct Capture(Mutex<String>);
        impl log::Log for Capture {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                true
            }
            fn log(&self, r: &log::Record<'_>) {
                self.0.lock().unwrap().push_str(&format!("{}\n", r.args()));
            }
            fn flush(&self) {}
        }
        static CAPTURE: Capture = Capture(Mutex::new(String::new()));
        log::set_logger(&CAPTURE).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
        if log::STATIC_MAX_LEVEL > log::LevelFilter::Debug {
            assert!(matches!(
                run(
                    "http://127.0.0.1:1",
                    "sentinel-secret",
                    &limits(),
                    "synthetic".into()
                ),
                Err(Error::Invalid(
                    "credential-bearing transport requires trace logging compiled out"
                ))
            ));
            return;
        }
        let (url, _, h) = mock(vec![
            (503, String::new(), "sentinel-secret".into()),
            (200, String::new(), page()),
            (200, String::new(), page()),
        ]);
        run(&url, "sentinel-secret", &limits(), "synthetic".into()).unwrap();
        h.join().unwrap();
        let (url, _, h) = mock(vec![(403, String::new(), "sentinel-secret".into())]);
        assert!(run(&url, "sentinel-secret", &limits(), "synthetic".into()).is_err());
        h.join().unwrap();
        let logs = CAPTURE.0.lock().unwrap();
        assert!(!logs.is_empty());
        assert!(!logs.contains("sentinel-secret"));
        assert!(!logs.contains("serviceKey"));
    }
    #[test]
    fn timeout_page_limit_and_total_bytes_fail() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let h = thread::spawn(move || {
            // The 10ms client deadline may expire before a connection is established.
            let mut stream = match accept_with_timeout(&listener, Duration::from_secs(5)) {
                Ok(stream) => stream,
                Err(error) if error.kind() == io::ErrorKind::TimedOut => return,
                Err(error) => panic!("mock server failed: {error}"),
            };
            let mut bytes = [0; 8192];
            let _ = stream.read(&mut bytes);
            thread::sleep(Duration::from_millis(60));
        });
        let mut l = limits();
        l.timeout = Duration::from_millis(10);
        l.retries = 0;
        assert!(matches!(
            run(&url, "sentinel-secret", &l, "synthetic".into()),
            Err(Error::Transport)
        ));
        h.join().unwrap();
        let body = page().replace("\"totalCount\":1", "\"totalCount\":2");
        let (url, _, h) = mock(vec![(200, String::new(), body)]);
        let mut l = limits();
        l.max_pages = 1;
        assert!(run(&url, "sentinel-secret", &l, "synthetic".into()).is_err());
        h.join().unwrap();
        let (url, _, h) = mock(vec![(200, String::new(), page())]);
        let mut l = limits();
        l.max_total_bytes = page().len() as u64;
        assert!(run(&url, "sentinel-secret", &l, "synthetic".into()).is_err());
        h.join().unwrap();
    }
}
