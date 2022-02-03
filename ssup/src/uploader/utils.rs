use async_stream::try_stream;
use bytes::{BufMut, Bytes, BytesMut};
use futures::Stream;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub(crate) fn read_chunk(mut file: File, len: usize) -> impl Stream<Item=anyhow::Result<Bytes>> {
    let mut buffer = vec![0u8; len];

    let mut buf = BytesMut::with_capacity(len);
    try_stream! {
        loop {
            let n = file.read(&mut buffer).await?;
            buf.put_slice(&buffer[..n]);
            if n == 0 {
                return;
            }
            yield buf.split().freeze();
        }
    }
}