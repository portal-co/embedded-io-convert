use core::error::Error;
use core::pin::pin;
use core::task::Poll;
use core::{pin::Pin, task::Context};
// use std::io;
use crate::read::internals::DidRead;
use either::Either;
use futures::{AsyncRead, Future};
use pin_project::pin_project;
// use crate::MutexFuture;
#[pin_project]
pub struct SimpleAsyncReader<R: embedded_io_async::Read>
where
    R: embedded_io_async::Read,
{
    state: internals::State<R>,
}
impl<R: embedded_io_async::Read> SimpleAsyncReader<R> {
    pub fn new(r: R) -> Self {
        return Self {
            state: internals::State::Idle(r, vec![]),
        };
    }
}
mod internals {
    use super::*;
    use alloc::{boxed::Box, vec::Vec};
    type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
    pub(crate) trait DidRead: embedded_io_async::Read + Sized {
        type T: Future<Output = (Self, Vec<u8>, Result<usize, Self::Error>)>;
        fn get_fut(this: Pin<&mut SimpleAsyncReader<Self>>, buf: &mut [u8]) -> Pin<Box<Self::T>>;
    }
    impl<R: embedded_io_async::Read> DidRead for R {
        type T = impl Future<Output = (Self, Vec<u8>, Result<usize, Self::Error>)>;
        fn get_fut(this: Pin<&mut SimpleAsyncReader<Self>>, buf: &mut [u8]) -> Pin<Box<Self::T>> {
            let proj = this.project();
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
                        (inner, internal_buf, res)
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
    // pub type DidRead<R: embedded_io_async::Read> =
    //     impl Future<Output = (R, Vec<u8>, Result<usize, R::Error>)>;
    pub enum State<R: embedded_io_async::Read> {
        Idle(R, Vec<u8>),
        Pending(Pin<Box<<R as DidRead>::T>>),
        Transitional,
    }
    // impl<R: embedded_io_async::Read> SimpleAsyncReader<R> {
    //     // #[define_opaque(DidRead)]
    //     pub(crate) fn get_fut(self: Pin<&mut Self>, buf: &mut [u8]) -> Pin<Box<DidRead<R>>> {
    //         let proj = self.project();
    //         let mut state = State::Transitional;
    //         std::mem::swap(proj.state, &mut state);
    //         let mut fut = match state {
    //             State::Idle(mut inner, mut internal_buf) => {
    //                 // tracing::debug!("getting new future...");
    //                 internal_buf.clear();
    //                 internal_buf.reserve(buf.len());
    //                 unsafe { internal_buf.set_len(buf.len()) }
    //                 let x = Box::pin(async move {
    //                     let res = inner.read(&mut internal_buf[..]).await;
    //                     (inner, internal_buf, res)
    //                 });
    //                 x
    //             }
    //             State::Pending(fut) => {
    //                 // tracing::debug!("polling existing future...");
    //                 fut
    //             }
    //             State::Transitional => unreachable!(),
    //         };
    //         return fut;
    //     }
    // }
    #[cfg(feature = "futures")]
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
        ) -> Poll<std::io::Result<usize>> {
            let mut fut = DidRead::get_fut(self.as_mut(), buf);
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
                    Poll::Ready(result.map_err(Into::into))
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
impl<R: embedded_io_async::Read<Error = E>, E> SimpleAsyncReader<R> {
    pub fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, E>> {
        let mut fut = DidRead::get_fut(self.as_mut(), buf);
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
                *proj.state = internals::State::Idle(inner, internal_buf);
                Poll::Ready(result.map_err(Into::into))
            }
            Poll::Pending => {
                // tracing::debug!("future was pending!");
                *proj.state = internals::State::Pending(fut);
                Poll::Pending
            }
        }
    }
}
