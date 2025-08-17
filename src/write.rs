use alloc::boxed::Box;
use core::error::Error;
// use std::io;
use core::task::Poll;
use core::{pin::Pin, task::Context};

use either::Either;
use embedded_io_async::Write;
use futures::{AsyncRead, AsyncWrite, Future};
use pin_project::pin_project;

use crate::write::internals::WriteExt;

// use crate::MutexFuture;

#[pin_project]
pub struct SimpleAsyncWriter<R: Write>
where
    R: embedded_io_async::Write,
{
    state: internals::State<R>,
}
impl<R: Write> SimpleAsyncWriter<R> {
    pub fn new(r: R) -> Self {
        return Self {
            state: internals::State::Idle(r),
        };
    }
}
type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
mod internals {
    use super::*;
    pub trait WriteExt: embedded_io_async::Write + Sized {
        type Written: Future<Output = (Self, Result<usize, Self::Error>)>;
        type Flushed: Future<Output = (Self, Result<(), Self::Error>)>;
        fn get_future(
            this: Pin<&mut SimpleAsyncWriter<Self>>,
            buf: Option<&[u8]>,
        ) -> Either<Pin<Box<Self::Written>>, Pin<Box<Self::Flushed>>>;
    }
    impl<R: embedded_io_async::Write> WriteExt for R{
        type Written = impl Future<Output = (Self, Result<usize, Self::Error>)>;
        type Flushed = impl Future<Output = (Self, Result<(), Self::Error>)>;
        fn get_future(
              this: Pin<&mut SimpleAsyncWriter<Self>>,
            buf: Option<&[u8]>,
        ) -> Either<Pin<Box<Self::Written>>, Pin<Box<Self::Flushed>>> {
            let proj = this.project();
            let mut state = State::Transitional;
            std::mem::swap(proj.state, &mut state);
            let buf = buf.map(|a| a.to_vec());
            let mut fut = match state {
                State::Idle(mut inner) => match buf {
                    Some(buf) => Either::Left(Box::pin(async move {
                        let res = inner.write(&buf).await;
                        (inner, res)
                    })),
                    None => Either::Right(Box::pin(async move {
                        let res = inner.flush().await;
                        (inner, res)
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
    // pub type Written<R: Write> = impl Future<Output = (R, Result<usize, R::Error>)>;
    // pub type Flushed<R: Write> = impl Future<Output = (R, Result<(), R::Error>)>;
    pub enum State<R: Write> {
        Idle(R),
        Pending(Pin<Box<<R as WriteExt>::Written>>),
        FPending(Pin<Box<<R as WriteExt>::Flushed>>),
        Transitional,
    }
    #[cfg(feature = "futures")]
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
        ) -> Poll<std::io::Result<usize>> {
            let mut fut = WriteExt::get_future( self.as_mut(),Some(buf));
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
                    Poll::Ready(result.map_err(|e| e.into()))
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

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            let mut fut =WriteExt::get_future( self.as_mut(),None);
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
                    Poll::Ready(result.map_err(|e| e.into()))
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

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            return Poll::Ready(Ok(()));
        }
    }
}
impl<R: embedded_io_async::Write> SimpleAsyncWriter<R> {
    pub fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, R::Error>> {
        let mut fut = WriteExt::get_future( self.as_mut(),Some(buf));
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
                *proj.state = internals::State::Idle(inner);
                Poll::Ready(result.map_err(|e| e.into()))
            }
            _ => {
                // tracing::debug!("future was pending!");
                *proj.state = match fut {
                    Either::Left(a) => internals::State::Pending(a),
                    Either::Right(b) => internals::State::FPending(b),
                };
                Poll::Pending
            }
        }
    }

    pub fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), R::Error>> {
        let mut fut = WriteExt::get_future( self.as_mut(),None);
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
                *proj.state = internals::State::Idle(inner);
                Poll::Ready(result.map_err(|e| e.into()))
            }
            _ => {
                // tracing::debug!("future was pending!");
                *proj.state = match fut {
                    Either::Left(a) => internals::State::Pending(a),
                    Either::Right(b) => internals::State::FPending(b),
                };
                Poll::Pending
            }
        }
    }
}
