#![allow(incomplete_features)]
#![feature(return_type_notation, impl_trait_in_assoc_type)]
#![no_std]
#[macro_use]
extern crate alloc;
#[cfg(feature = "std")]
#[macro_use]
extern crate std;
use core::pin::Pin;
use embedded_io_async::ErrorType;
#[cfg(feature = "futures")]
use futures::{AsyncRead, AsyncSeek, AsyncWrite, Future};
pub mod mutex;
pub mod read;
pub mod seek;
pub mod write;
#[cfg(feature = "futures")]
pub use embedded_io_adapters::futures_03 as from;
#[cfg(feature = "futures")]
pub fn read_writer<
    'a,
    E: embedded_io_async::Read<read(..): Send>
        + embedded_io_async::Write<write(..): Send, flush(..): Send>
        + Send
        + 'a,
>(
    a: E,
) -> impl AsyncRead + AsyncWrite + Unpin + Send + 'a
where
    <E as ErrorType>::Error: Into<std::io::Error> + Send,
{
    let a = mutex::Mutexed::new(a);
    let b = a.clone();
    let a = read::SimpleAsyncReader::new(a);
    let b = write::SimpleAsyncWriter::new(b);
    return merge_io::MergeIO::new(a, b);
}
#[cfg(feature = "futures")]
pub fn read_writer_unsend<'a, E: embedded_io_async::Read + embedded_io_async::Write + Send + 'a>(
    a: E,
) -> impl AsyncRead + AsyncWrite + Unpin + 'a
where
    <E as ErrorType>::Error: Into<std::io::Error>,
{
    let a = mutex::Mutexed::new(a);
    let b = a.clone();
    let a = read::SimpleAsyncReader::new(a);
    let b = write::SimpleAsyncWriter::new(b);
    return merge_io::MergeIO::new(a, b);
}
#[cfg(feature = "futures")]
pub fn read_write_seeeker<
    'a,
    E: embedded_io_async::Read<read(..): Send>
        + embedded_io_async::Write<write(..): Send, flush(..): Send>
        + embedded_io_async::Seek<seek(..): Send>
        + Send
        + 'a,
>(
    a: E,
) -> impl AsyncRead + AsyncWrite + AsyncSeek + Unpin + Send + 'a
where
    <E as ErrorType>::Error: Into<std::io::Error> + Send,
{
    let a = mutex::Mutexed::new(a);
    let b = a.clone();
    let a = read_writer(a);
    let b = seek::SimpleAsyncSeeker::new(b);
    return MergeSeek {
        readwrite: a,
        seek: b,
    };
}
#[cfg(feature = "futures")]
pub fn read_write_seeeker_unsend<
    'a,
    E: embedded_io_async::Read + embedded_io_async::Write + embedded_io_async::Seek + Send + 'a,
>(
    a: E,
) -> impl AsyncRead + AsyncWrite + AsyncSeek + Unpin + 'a
where
    <E as ErrorType>::Error: Into<std::io::Error> + Send,
{
    let a = mutex::Mutexed::new(a);
    let b = a.clone();
    let a = read_writer_unsend(a);
    let b = seek::SimpleAsyncSeeker::new(b);
    return MergeSeek {
        readwrite: a,
        seek: b,
    };
}
pub struct MergeSeek<RW, S> {
    pub readwrite: RW,
    pub seek: S,
}
#[cfg(feature = "futures")]
impl<RW: AsyncRead + Unpin, S: Unpin> AsyncRead for MergeSeek<RW, S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        return AsyncRead::poll_read(Pin::new(&mut self.get_mut().readwrite), cx, buf);
    }
    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &mut [std::io::IoSliceMut<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        return AsyncRead::poll_read_vectored(Pin::new(&mut self.get_mut().readwrite), cx, bufs);
    }
}
#[cfg(feature = "futures")]
impl<RW: AsyncWrite + Unpin, S: Unpin> AsyncWrite for MergeSeek<RW, S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        return AsyncWrite::poll_write(Pin::new(&mut self.get_mut().readwrite), cx, buf);
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        return AsyncWrite::poll_flush(Pin::new(&mut self.get_mut().readwrite), cx);
    }
    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        return AsyncWrite::poll_close(Pin::new(&mut self.get_mut().readwrite), cx);
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        return AsyncWrite::poll_write_vectored(Pin::new(&mut self.get_mut().readwrite), cx, bufs);
    }
}
#[cfg(feature = "futures")]
impl<RW: Unpin, S: AsyncSeek + Unpin> AsyncSeek for MergeSeek<RW, S> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        pos: std::io::SeekFrom,
    ) -> std::task::Poll<std::io::Result<u64>> {
        return AsyncSeek::poll_seek(Pin::new(&mut self.get_mut().seek), cx, pos);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
}
