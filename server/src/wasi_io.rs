use std::io;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};
use std::task::{Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite};
use wasi::{
    cli::{
        stdin::{InputStream, get_stdin},
        stdout::{OutputStream, get_stdout},
    },
    io::streams::Pollable,
};

#[derive(Default)]
struct WasiPollableRegistry {
    pollables: Mutex<Vec<(Pollable, Waker)>>,
}

impl WasiPollableRegistry {
    fn register(&self, pollable: Pollable, waker: Waker) {
        self.pollables.lock().unwrap().push((pollable, waker));
    }

    fn poll_all(&self) {
        let mut ready_wakers = Vec::new();
        let mut pollables = self.pollables.lock().unwrap();

        let mut i = 0;
        while i < pollables.len() {
            let (pollable, waker) = &pollables[i];
            if pollable.ready() {
                ready_wakers.push(waker.clone());
                pollables.remove(i);
            } else {
                i += 1;
            }
        }

        for waker in ready_wakers {
            waker.wake();
        }
    }
}

static REGISTRY: LazyLock<WasiPollableRegistry> = LazyLock::new(WasiPollableRegistry::default);

pub fn init_wasi_runtime() -> io::Result<()> {
    let rt = tokio::runtime::Handle::current();

    rt.spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
        loop {
            interval.tick().await;
            REGISTRY.poll_all();
        }
    });

    Ok(())
}

pub fn wasi_io() -> (AsyncInputStream, AsyncOutputStream) {
    let input = AsyncInputStream { inner: get_stdin() };
    let output = AsyncOutputStream {
        inner: get_stdout(),
    };
    (input, output)
}

pub struct AsyncInputStream {
    inner: InputStream,
}

impl AsyncRead for AsyncInputStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.inner.read(buf.remaining() as u64) {
            Ok(bytes) => {
                if !bytes.is_empty() {
                    buf.put_slice(&bytes);
                    Poll::Ready(Ok(()))
                } else {
                    let pollable = self.inner.subscribe();
                    REGISTRY.register(pollable, cx.waker().clone());
                    Poll::Pending
                }
            }
            Err(e) => Poll::Ready(Err(io::Error::other(e))),
        }
    }
}

pub struct AsyncOutputStream {
    inner: OutputStream,
}

impl AsyncWrite for AsyncOutputStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.inner.check_write() {
            Ok(writable_len) => {
                if writable_len > 0 {
                    let bytes_to_write = buf.len().min(writable_len as usize);
                    match self.inner.write(&buf[0..bytes_to_write]) {
                        Ok(_) => Poll::Ready(Ok(bytes_to_write)),
                        Err(e) => Poll::Ready(Err(io::Error::other(e))),
                    }
                } else {
                    let pollable = self.inner.subscribe();
                    REGISTRY.register(pollable, cx.waker().clone());
                    Poll::Pending
                }
            }
            Err(e) => Poll::Ready(Err(io::Error::other(e))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.inner.flush() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(io::Error::other(e))),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
