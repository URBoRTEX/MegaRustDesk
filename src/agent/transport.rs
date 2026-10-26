//! Bounded newline framing for the private, authenticated loopback transport.
use std::io::{self, BufRead, Read, Write};

pub fn write_message(writer: &mut impl Write, message: &[u8]) -> io::Result<()> {
    // Serialize before touching the socket: no separate JSON field/trailer writes.
    let mut framed = Vec::with_capacity(message.len() + 1);
    framed.extend_from_slice(message);
    framed.push(b'\n');
    writer.write_all(&framed)
}

pub fn read_message(reader: &mut impl BufRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut message = Vec::new();
    reader.take(limit.saturating_add(1) as u64).read_until(b'\n', &mut message)?;
    if message.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MCP message exceeds size limit"));
    }
    if !message.ends_with(b"\n") {
        let detail = if message.is_empty() { "empty MCP response" } else { "incomplete MCP message" };
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, detail));
    }
    Ok(message)
}

pub fn retry_read_only<T>(name: &str, mut request: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let attempts = if matches!(name, "screenshot" | "session_status") { 3 } else { 1 };
    for attempt in 0..attempts {
        match request() {
            Err(error) if attempt + 1 < attempts && matches!(error.kind(),
                io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset |
                io::ErrorKind::UnexpectedEof | io::ErrorKind::TimedOut |
                io::ErrorKind::WouldBlock | io::ErrorKind::BrokenPipe | io::ErrorKind::Interrupted
            ) => std::thread::sleep(std::time::Duration::from_millis(40 * (attempt as u64 + 1))),
            result => return result,
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{BufReader, Cursor}, net::{TcpListener, TcpStream}, thread};

    #[test]
    fn empty_eof_is_not_a_response() {
        let error = read_message(&mut Cursor::new(Vec::<u8>::new()), 100).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn truncated_response_is_not_success() {
        let error = read_message(&mut Cursor::new(b"{\"content\":["), 100).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert!(error.to_string().contains("incomplete"));
    }

    #[test]
    fn oversized_response_is_rejected_before_parsing() {
        let error = read_message(&mut Cursor::new(b"123456\n"), 5).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn complete_response_at_limit_is_accepted() {
        assert_eq!(read_message(&mut Cursor::new(b"1234\n"), 5).unwrap(), b"1234\n");
    }

    #[test]
    fn large_fragmented_loopback_response_arrives_intact() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let expected = vec![b'x'; 600_000];
        let sent = expected.clone();
        let sender = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            for chunk in sent.chunks(4093) { socket.write_all(chunk).unwrap(); }
            socket.write_all(b"\n").unwrap();
        });
        let socket = TcpStream::connect(address).unwrap();
        let received = read_message(&mut BufReader::new(socket), 1024 * 1024).unwrap();
        assert_eq!(&received[..received.len() - 1], expected);
        sender.join().unwrap();
    }

    #[test]
    fn write_errors_are_returned() {
        struct Failed;
        impl Write for Failed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        assert_eq!(write_message(&mut Failed, b"{} ").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn screenshot_recovers_from_incomplete_reply() {
        let mut calls = 0;
        let result = retry_read_only("screenshot", || {
            calls += 1;
            if calls < 3 { Err(io::ErrorKind::UnexpectedEof.into()) } else { Ok(42) }
        }).unwrap();
        assert_eq!(result, 42);
        assert_eq!(calls, 3);
    }

    #[test]
    fn input_and_control_are_never_repeated() {
        for name in ["click", "double_click", "scroll", "drag", "hotkey", "type_text", "pause", "stop", "select_display", "controller_closed"] {
            let mut calls = 0;
            let _: io::Result<()> = retry_read_only(name, || { calls += 1; Err(io::ErrorKind::ConnectionReset.into()) });
            assert_eq!(calls, 1, "{name}");
        }
    }

    #[test]
    fn read_retry_is_bounded_and_does_not_retry_missing_endpoint() {
        for (kind, expected) in [(io::ErrorKind::ConnectionAborted, 3), (io::ErrorKind::ConnectionRefused, 1), (io::ErrorKind::InvalidData, 1)] {
            let mut calls = 0;
            let _: io::Result<()> = retry_read_only("session_status", || { calls += 1; Err(kind.into()) });
            assert_eq!(calls, expected);
        }
    }

    #[test]
    fn complete_application_error_is_returned_once() {
        let mut calls = 0;
        let result = retry_read_only("screenshot", || { calls += 1; Ok("paused") }).unwrap();
        assert_eq!(result, "paused");
        assert_eq!(calls, 1);
    }
}
