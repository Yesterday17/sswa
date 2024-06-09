use async_stream::try_stream;
use bytes::{BufMut, Bytes, BytesMut};
use futures::Stream;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub(crate) fn read_chunk(
    mut file: File,
    chunk_size: usize,
) -> impl Stream<Item = anyhow::Result<Bytes>> {
    let mut buffer = vec![0u8; chunk_size];

    let mut buf = BytesMut::with_capacity(chunk_size);
    try_stream! {
        loop {
            let n = file.read(&mut buffer).await?;
            let remaining = chunk_size - buf.len();
            if remaining >= n {
                buf.put_slice(&buffer[..n]);
            } else {
                buf.put_slice(&buffer[..remaining]);
                yield buf.split().freeze();
                buf.put_slice(&buffer[remaining..n]);
            }
            if n == 0 {
                yield buf.split().freeze();
                return;
            }
        }
    }
}
