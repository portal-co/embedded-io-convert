use std::error::Error;
use std::io;
use std::task::Poll;
use std::{pin::Pin, task::Context};

use embedded_io_async::{Seek, Write};
use futures::{AsyncRead, AsyncSeek, AsyncWrite, Future};
use pin_project::pin_project;

// use crate::MutexFuture;

#[pin_project]
pub struct SimpleAsyncSeeker<R: Seek<Error: Into<std::io::Error>>>
where
    R: embedded_io_async::Seek,
{
    state: internals::State<R>,
}
impl<R: Seek<Error: Into<std::io::Error>>> SimpleAsyncSeeker<R> {
    pub fn new(r: R) -> Self {
        return Self {
            state: internals::State::Idle(r),
        };
    }
}
mod internals{use super::*;
type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

type Seeked<R: Seek<Error: Into<std::io::Error>>> = impl Future<Output = (R, io::Result<u64>)>;

pub enum State<R: Seek<Error: Into<std::io::Error>>> {
    Idle(R),
    Pending(Pin<Box<Seeked<R>>>),
    Transitional,
}
impl<R: Seek<Error: Into<std::io::Error>>> SimpleAsyncSeeker<R>{
    fn get_seeked(self: Pin<&mut Self>, pos: io::SeekFrom) -> Pin<Box<Seeked<R>>>{
        let proj = self.project();
        let mut state = State::Transitional;
        std::mem::swap(proj.state, &mut state);
        // let buf = buf.to_vec();
        let mut fut = match state {
            State::Idle(mut inner) => Box::pin(async move {
                let res = inner.seek(pos.into()).await;
                (inner, res.map_err(|e| e.into()))
            }),
            State::Pending(fut) => {
                // tracing::debug!("polling existing future...");
                fut
            }
            State::Transitional => unreachable!(),
        };
        return fut;
    }
}
impl<R> AsyncSeek for SimpleAsyncSeeker<R>
where
    // new: R must now be `'static`, since it's captured
    // by the future which is, itself, `'static`.
    R: embedded_io_async::Seek,
    R::Error: Into<std::io::Error>,
{
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: io::SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let mut fut  = self.as_mut().get_seeked(pos);
        let proj = self.project();

        match fut.as_mut().poll(cx) {
            Poll::Ready((inner, result)) => {
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
            Poll::Pending => {
                // tracing::debug!("future was pending!");
                *proj.state = State::Pending(fut);
                Poll::Pending
            }
        }
    }
}
}