use std::error::Error;
use std::io;
use std::task::Poll;
use std::{pin::Pin, task::Context};

use either::Either;
use embedded_io_async::Write;
use futures::{AsyncRead, AsyncWrite, Future};
use pin_project::pin_project;

// use crate::MutexFuture;

#[pin_project]
pub struct SimpleAsyncWriter<R: Write<Error: Into<std::io::Error>>>
where
    R: embedded_io_async::Write,
{
    state: internals::State<R>,
}
impl<R: Write<Error: Into<std::io::Error>>> SimpleAsyncWriter<R> {
    pub fn new(r: R) -> Self {
        return Self {
            state: internals::State::Idle(r),
        };
    }
}
type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
mod internals {
    use super::*;
    type Written<R: Write<Error: Into<std::io::Error>>> =
        impl Future<Output = (R, io::Result<usize>)>;
    type Flushed<R: Write<Error: Into<std::io::Error>>> = impl Future<Output = (R, io::Result<()>)>;
    pub enum State<R: Write<Error: Into<std::io::Error>>> {
        Idle(R),
        Pending(Pin<Box<Written<R>>>),
        FPending(Pin<Box<Flushed<R>>>),
        Transitional,
    }
    impl<R: Write<Error: Into<std::io::Error>>> SimpleAsyncWriter<R> {
        fn get_future(
            self: Pin<&mut Self>,
            buf: Option<&[u8]>,
        ) -> Either<Pin<Box<Written<R>>>, Pin<Box<Flushed<R>>>> {
            let proj = self.project();
            let mut state = State::Transitional;
            std::mem::swap(proj.state, &mut state);
            let buf = buf.map(|a| a.to_vec());
            let mut fut = match state {
                State::Idle(mut inner) => match buf {
                    Some(buf) => Either::Left(Box::pin(async move {
                        let res = inner.write(&buf).await;
                        (inner, res.map_err(|e| e.into()))
                    })),
                    None => Either::Right(Box::pin(async move {
                        let res = inner.flush().await;
                        (inner, res.map_err(|e| e.into()))
                    })),
                },
                State::Pending(fut) => {
                    // tracing::debug!("polling existing future...");
                    Either::Left(fut)
                }
                State::Transitional => unreachable!(),
                State::FPending(pin) => Either::Right(pin),
            };
            return fut;
        }
    }
    impl<R> AsyncWrite for SimpleAsyncWriter<R>
    where
        // new: R must now be `'static`, since it's captured
        // by the future which is, itself, `'static`.
        R: embedded_io_async::Write,
        R::Error: Into<std::io::Error>,
    {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let mut fut = self.as_mut().get_future(Some(buf));
            let proj = self.project();

            match match fut.as_mut() {
                Either::Left(a) => a.as_mut().poll(cx).map(Either::Left),
                Either::Right(b) => b.as_mut().poll(cx).map(Either::Right),
            } {
                Poll::Ready(Either::Left((inner, result))) => {
                    // tracing::debug!("future was ready!");
                    // if let Ok(n) = &result {
                    //     let n = *n;
                    //     // unsafe { internal_buf.set_len(n) }

                    //     // let dst = &mut buf[..n];
                    //     // let src = &internal_buf[..];
                    //     // dst.copy_from_slice(src);
                    // } else {
                    //     // unsafe { internal_buf.set_len(0) }
                    // }
                    *proj.state = State::Idle(inner);
                    Poll::Ready(result)
                }
                _ => {
                    // tracing::debug!("future was pending!");
                    *proj.state = match fut {
                        Either::Left(a) => State::Pending(a),
                        Either::Right(b) => State::FPending(b),
                    };
                    Poll::Pending
                }
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let mut fut = self.as_mut().get_future(None);
            let proj = self.project();

            match match fut.as_mut() {
                Either::Left(a) => a.as_mut().poll(cx).map(Either::Left),
                Either::Right(b) => b.as_mut().poll(cx).map(Either::Right),
            } {
                Poll::Ready(Either::Right((inner, result))) => {
                    // tracing::debug!("future was ready!");
                    // if let Ok(n) = &result {
                    //     let n = *n;
                    //     // unsafe { internal_buf.set_len(n) }

                    //     // let dst = &mut buf[..n];
                    //     // let src = &internal_buf[..];
                    //     // dst.copy_from_slice(src);
                    // } else {
                    //     // unsafe { internal_buf.set_len(0) }
                    // }
                    *proj.state = State::Idle(inner);
                    Poll::Ready(result)
                }
                _ => {
                    // tracing::debug!("future was pending!");
                    *proj.state = match fut {
                        Either::Left(a) => State::Pending(a),
                        Either::Right(b) => State::FPending(b),
                    };
                    Poll::Pending
                }
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            return Poll::Ready(Ok(()));
        }
    }
}
