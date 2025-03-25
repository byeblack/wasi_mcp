use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub struct MonitoredStream<R, W> {
    inner_read: R,
    inner_write: W,
    label: &'static str,
}

#[allow(dead_code)]
impl<R, W> MonitoredStream<R, W> {
    pub fn new(inner_read: R, inner_write: W, label: &'static str) -> Self {
        Self {
            inner_read,
            inner_write,
            label,
        }
    }
}

impl<R: AsyncRead + Unpin, W> AsyncRead for MonitoredStream<R, W> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before_len = buf.filled().len();
        let inner_read = unsafe { self.as_mut().get_unchecked_mut() };
        let poll = Pin::new(&mut inner_read.inner_read).poll_read(cx, buf);

        if let Poll::Ready(Ok(())) = poll {
            let new_data = &buf.filled()[before_len..];
            if !new_data.is_empty() {
                tracing::trace!(
                    "[{}] READ {} bytes: {:?}",
                    self.label,
                    new_data.len(),
                    bytes::Bytes::copy_from_slice(new_data)
                );
            }
        }

        poll
    }
}

impl<R, W: AsyncWrite + Unpin> AsyncWrite for MonitoredStream<R, W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let inner_write = unsafe { self.as_mut().get_unchecked_mut() };
        let poll = Pin::new(&mut inner_write.inner_write).poll_write(cx, buf);

        if let Poll::Ready(Ok(n)) = poll {
            if n > 0 {
                tracing::trace!(
                    "[{}] WRITE {} bytes: {:?}",
                    self.label,
                    n,
                    bytes::Bytes::copy_from_slice(&buf[..n])
                );
            }
        }

        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let inner_write = unsafe { self.as_mut().get_unchecked_mut() };
        Pin::new(&mut inner_write.inner_write).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let inner_write = unsafe { self.as_mut().get_unchecked_mut() };
        Pin::new(&mut inner_write.inner_write).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct MockReader {
        data: Cursor<Vec<u8>>,
    }

    impl MockReader {
        fn new(data: &[u8]) -> Self {
            Self {
                data: Cursor::new(data.to_vec()),
            }
        }
    }

    impl AsyncRead for MockReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let available = self.data.get_ref().len() as u64 - self.data.position();
            if available == 0 {
                return Poll::Ready(Ok(()));
            }

            let to_read = std::cmp::min(available as usize, buf.remaining());
            let pos = self.data.position() as usize;
            let data = &self.data.get_ref()[pos..pos + to_read];
            buf.put_slice(data);
            self.data.set_position(pos as u64 + to_read as u64);

            Poll::Ready(Ok(()))
        }
    }

    struct MockWriter {
        data: Vec<u8>,
    }

    impl MockWriter {
        fn new() -> Self {
            Self { data: Vec::new() }
        }

        fn written_data(&self) -> &[u8] {
            &self.data
        }
    }

    impl AsyncWrite for MockWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.data.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_monitored_stream_read() {
        let test_data = b"hello world";
        let reader = MockReader::new(test_data);
        let writer = ();

        let mut monitored = MonitoredStream::new(reader, writer, "TEST_READ");
        let mut buffer = vec![0u8; 20];

        let bytes_read = monitored.read(&mut buffer).await.unwrap();

        assert_eq!(bytes_read, test_data.len());
        assert_eq!(&buffer[..bytes_read], test_data);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_monitored_stream_write() {
        let reader = ();
        let writer = MockWriter::new();

        let mut monitored = MonitoredStream::new(reader, writer, "TEST_WRITE");
        let test_data = b"hello world";

        let bytes_written = monitored.write(test_data).await.unwrap();
        monitored.flush().await.unwrap();

        assert_eq!(bytes_written, test_data.len());
        assert_eq!(monitored.inner_write.written_data(), test_data);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_monitored_stream_read_sync() {
        let test_data = b"hello world";
        let reader = MockReader::new(test_data);
        let writer = ();

        let mut monitored = MonitoredStream::new(reader, writer, "TEST_READ_SYNC");
        let mut buffer = vec![0u8; 20];

        let bytes_read = monitored.read(&mut buffer).await.unwrap();

        assert_eq!(bytes_read, test_data.len());
        assert_eq!(&buffer[..bytes_read], test_data);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_monitored_stream_write_sync() {
        let reader = ();
        let writer = MockWriter::new();

        let mut monitored = MonitoredStream::new(reader, writer, "TEST_WRITE_SYNC");
        let test_data = b"hello world";

        let bytes_written = monitored.write(test_data).await.unwrap();
        monitored.flush().await.unwrap();

        assert_eq!(bytes_written, test_data.len());
        assert_eq!(monitored.inner_write.written_data(), test_data);
    }
}
