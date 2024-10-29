use std::error::Error;
use std::task::Poll;
use std::{io, pin::pin};
use std::{pin::Pin, task::Context};

use either::Either;
use futures::{AsyncRead, Future};
use pin_project::pin_project;

// use crate::MutexFuture;

#[pin_project]
pub struct SimpleAsyncReader<R: embedded_io_async::Read<Error: Into<std::io::Error>>>
where
    R: embedded_io_async::Read,
{
    state: internals::State<R>,
}
impl<R: embedded_io_async::Read<Error: Into<std::io::Error>>> SimpleAsyncReader<R> {
    pub fn new(r: R) -> Self {
        return Self {
            state: internals::State::Idle(r, vec![]),
        };
    }
}
mod internals {
    use super::*;
    type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

    type DidRead<R: embedded_io_async::Read<Error: Into<std::io::Error>>> =
        impl Future<Output = (R, Vec<u8>, io::Result<usize>)>;

    pub enum State<R: embedded_io_async::Read<Error: Into<std::io::Error>>> {
        Idle(R, Vec<u8>),
        Pending(Pin<Box<DidRead<R>>>),
        Transitional,
    }
    impl<R: embedded_io_async::Read<Error: Into<std::io::Error>>> SimpleAsyncReader<R> {
        fn get_fut(self: Pin<&mut Self>, buf: &mut [u8]) -> Pin<Box<DidRead<R>>> {
            let proj = self.project();
            let mut state = State::Transitional;
            std::mem::swap(proj.state, &mut state);

            let mut fut = match state {
                State::Idle(mut inner, mut internal_buf) => {
                    // tracing::debug!("getting new future...");
                    internal_buf.clear();
                    internal_buf.reserve(buf.len());
                    unsafe { internal_buf.set_len(buf.len()) }
                    let x = Box::pin(async move {
                        let res = inner.read(&mut internal_buf[..]).await;
                        (inner, internal_buf, res.map_err(|e| e.into()))
                    });

                    x
                }
                State::Pending(fut) => {
                    // tracing::debug!("polling existing future...");
                    fut
                }
                State::Transitional => unreachable!(),
            };
            return fut;
        }
    }
    impl<R> AsyncRead for SimpleAsyncReader<R>
    where
        // new: R must now be `'static`, since it's captured
        // by the future which is, itself, `'static`.
        R: embedded_io_async::Read,
        R::Error: Into<std::io::Error>,
    {
        // #[tracing::instrument(skip(self, buf))]
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let mut fut = self.as_mut().get_fut(buf);
            let proj = self.project();

            match fut.as_mut().poll(cx) {
                Poll::Ready((inner, mut internal_buf, result)) => {
                    // tracing::debug!("future was ready!");
                    if let Ok(n) = &result {
                        let n = *n;
                        unsafe { internal_buf.set_len(n) }

                        let dst = &mut buf[..n];
                        let src = &internal_buf[..];
                        dst.copy_from_slice(src);
                    } else {
                        unsafe { internal_buf.set_len(0) }
                    }
                    *proj.state = State::Idle(inner, internal_buf);
                    Poll::Ready(result)
                }
                Poll::Pending => {
                    // tracing::debug!("future was pending!");
                    *proj.state = State::Pending(fut);
                    Poll::Pending
                }
            }
        }
    }
}
