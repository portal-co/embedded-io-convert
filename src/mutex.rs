use alloc::sync::Arc;

use embedded_io_async::{ErrorType, Read, Seek, Write};
use no_std_async::Mutex;
// use futures::lock::Mutex;

pub struct Mutexed<T>(pub Arc<Mutex<T>>);
impl<T> Clone for Mutexed<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: ErrorType> ErrorType for Mutexed<T> {
    type Error = T::Error;
}
impl<T: Read> Read for Mutexed<T> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut lock = self.0.lock().await;
        return lock.read(buf).await;
    }
}
impl<T: Write> Write for Mutexed<T> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut lock = self.0.lock().await;
        return lock.write(buf).await;
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        let mut lock = self.0.lock().await;
        return lock.flush().await;
    }
}
impl<T: Seek> Seek for Mutexed<T> {
    async fn seek(&mut self, pos: embedded_io_async::SeekFrom) -> Result<u64, Self::Error> {
        let mut lock = self.0.lock().await;
        return lock.seek(pos).await;
    }
}
impl<T> Mutexed<T> {
    pub fn new(t: T) -> Self {
        return Self(Arc::new(Mutex::new(t)));
    }
}
