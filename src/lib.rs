use async_std::sync::Mutex;
use async_stream::try_stream;
use cid::Cid;
use futures::future::BoxFuture;
use futures::AsyncRead;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use std::pin::Pin;
use std::task::ready;
use std::task::{Context, Poll};

use crate::block_cid::assert_block_cid;
use crate::car_block::decode_block;
use crate::car_header::read_car_header;
pub use crate::car_header::CarHeader;
use crate::car_header::StreamEnd;
use crate::error::CarDecodeError;

mod block_cid;
mod car_block;
mod car_header;
mod carv1_header;
mod carv2_header;
pub mod error;
mod varint;

/// CARv1 consists of:
/// - TODO
///
/// ```nn
/// [-------header---------][---------------data---------------]
/// [varint][DAG-CBOR block][varint|CID|block][varint|CID|block]
/// ```
///
/// ## Header
/// First

/// CARv2 consists of:
/// - 11-byte pragma
/// - 40-byte header with characteristics and locations
/// - CARv1 data payload, including header, roots and sequence of CID:Bytes pairs
/// - Optional index for fast lookup
///
/// ```nn
/// [pragma][v2 header][opt padding][CARv1][opt padding][opt index]
/// ```
///

struct CarDecodeBlockStreamer<'a, R> {
    // r: &'a mut R,
    pub header: CarHeader,
    pub read_bytes: usize,
    validate_block_hash: bool,
    decode_header_future: Option<DecodeBlockFuture<'a, R>>,
}

type DecodeBlockFuture<'a, R> =
    BoxFuture<'a, Result<(&'a mut R, Cid, Vec<u8>, usize), CarDecodeError>>;

impl<'a, R> Stream for CarDecodeBlockStreamer<'a, R>
where
    R: AsyncRead + Send + Unpin + 'a,
{
    type Item = Result<(Cid, Vec<u8>), CarDecodeError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = Pin::into_inner(self);

        match &mut me.decode_header_future {
            Some(decode_future) => match decode_future.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Ok((r, cid, block, block_len))) => {
                    if let StreamEnd::AfterNBytes(blocks_len) = me.header.eof_stream {
                        if me.read_bytes >= blocks_len {
                            return Poll::Ready(None);
                        }
                    }
                    if me.validate_block_hash {
                        assert_block_cid(&cid, &block)?;
                    }
                    me.read_bytes += block_len;
                    me.decode_header_future = Some(Box::pin(decode_block(r)));
                    Poll::Ready(Some(Ok((cid, block))))
                }
                Poll::Ready(Err(CarDecodeError::BlockStartEOF))
                    if me.header.eof_stream == StreamEnd::OnBlockEOF =>
                {
                    return Poll::Ready(None)
                }
                Poll::Ready(Err(err)) => {
                    me.decode_header_future = None;
                    Poll::Ready(Some(Err(err)))
                }
            },
            None => Poll::Ready(None),
        }
    }
}

impl<'a, R> CarDecodeBlockStreamer<'a, R>
where
    R: AsyncRead + Send + Unpin,
{
    pub async fn new(
        r: &'a mut R,
        validate_block_hash: bool,
    ) -> Result<CarDecodeBlockStreamer<'a, R>, CarDecodeError> {
        let header = read_car_header(r).await?;
        println!("header {:?}", header);
        let dec = decode_block(r).await;
        return Ok(CarDecodeBlockStreamer {
            header,
            read_bytes: 0,
            validate_block_hash,
            decode_header_future: Some(Box::pin(async { dec })),
        });
    }
}

pub async fn decode_car<R: AsyncRead + Unpin + Send>(
    r: &mut R,
    validate_block_hash: bool,
) -> Result<(Vec<(Cid, Vec<u8>)>, CarHeader), CarDecodeError> {
    let mut decoder = CarDecodeBlockStreamer::new(r, validate_block_hash).await?;
    let mut items: Vec<(Cid, Vec<u8>)> = vec![];

    while let Some(item) = decoder.next().await {
        let item = item?;
        println!("block {:?}", item);
        items.push(item);
    }

    Ok((items, decoder.header))
}

pub async fn decode_car_no_stream<R: AsyncRead + Unpin>(
    r: &mut R,
    validate_block_hash: bool,
) -> Result<(Vec<(Cid, Vec<u8>)>, CarHeader), CarDecodeError> {
    let header = read_car_header(r).await?;
    let mut items: Vec<(Cid, Vec<u8>)> = vec![];
    let mut read_bytes = 0;

    loop {
        let (_, cid, block, block_len) = match decode_block(r).await {
            Ok(data) => data,
            Err(CarDecodeError::BlockStartEOF) if header.eof_stream == StreamEnd::OnBlockEOF => {
                break
            }
            Err(err) => return Err(err),
        };

        if validate_block_hash {
            assert_block_cid(&cid, &block)?;
        }

        items.push((cid, block));

        read_bytes += block_len;
        if let StreamEnd::AfterNBytes(blocks_len) = header.eof_stream {
            if read_bytes >= blocks_len {
                break;
            }
        }
    }

    Ok((items, header))
}

fn decode_car_async_stream<R: AsyncRead + Unpin>(
    mut r: R,
    validate_block_hash: bool,
) -> impl Stream<Item = Result<(Cid, Vec<u8>), CarDecodeError>> {
    try_stream! {
        let header = read_car_header(&mut r).await?;
        let mut read_bytes = 0;

        loop {
            let (_, cid, block, block_len) = match decode_block(&mut r).await {
                Ok(data) => data,
                Err(CarDecodeError::BlockStartEOF) if header.eof_stream == StreamEnd::OnBlockEOF => {
                    break
                }
                Err(err) => Err(err)?,
            };

            if validate_block_hash {
                assert_block_cid(&cid, &block)?;
            }

            yield (cid, block);

            read_bytes += block_len;
            if let StreamEnd::AfterNBytes(blocks_len) = header.eof_stream {
                if read_bytes >= blocks_len {
                    break;
                }
            }
        }

    }
}

// pub async fn decode_car_2<R: AsyncRead + Send + Unpin>(
//     r: R,
//     validate_block_hash: bool,
// ) -> Result<(Vec<(Cid, Vec<u8>)>, CarHeader), CarDecodeError> {
//     let mut decoder = decode_car_async_stream(r, validate_block_hash);

//     let items = decoder.collect::<Vec<(Cid, Vec<u8>)>>().await;

//     let mut items = vec![];

//     while let Some(item) = Pin::new(&mut decoder).next().await {
//         let item = item?;
//         println!("block {:?}", item);
//         items.push(item);
//     }

//     Ok((items, decoder.header))
// }

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::carv1_header::CarV1Header;

    use super::*;

    #[tokio::test]
    async fn decode_carv1_helloworld_no_stream() {
        let car_filepath = "./tests/custom_fixtures/helloworld.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
        let (blocks, header) = decode_car_no_stream(&mut file, true).await.unwrap();

        let root_cid = Cid::from_str("QmUU2HcUBVSXkfWPUc3WUSeCMrWWeEJTuAgR9uyWBhh9Nf").unwrap();
        let root_block = hex::decode("0a110802120b68656c6c6f776f726c640a180b").unwrap();

        assert_eq!(blocks, vec!((root_cid, root_block)));
        assert_eq!(
            header.header_v1,
            CarV1Header {
                version: 1,
                roots: Some(vec!(root_cid))
            }
        )
    }

    #[tokio::test]
    async fn decode_carv1_helloworld_stream() {
        let car_filepath = "./tests/custom_fixtures/helloworld.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
        let (blocks, header) = decode_car(&mut file, true).await.unwrap();

        let root_cid = Cid::from_str("QmUU2HcUBVSXkfWPUc3WUSeCMrWWeEJTuAgR9uyWBhh9Nf").unwrap();
        let root_block = hex::decode("0a110802120b68656c6c6f776f726c640a180b").unwrap();

        assert_eq!(blocks, vec!((root_cid, root_block)));
        assert_eq!(
            header.header_v1,
            CarV1Header {
                version: 1,
                roots: Some(vec!(root_cid))
            }
        )
    }

    #[tokio::test]
    async fn decode_carv1_basic() {
        let car_filepath = "./tests/spec_fixtures/carv1-basic.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
        decode_car(&mut file, true).await.unwrap();
    }

    #[tokio::test]
    async fn decode_carv2_basic() {
        // 0aa16776657273696f6e02  - v2 pragma
        // 00000000000000000000000000000000  - v2 header characteristics
        // 3300000000000000  - v2 header data_offset
        // c001000000000000  - v2 header data_size
        // f301000000000000  - v2 header index_offset
        // 38a265726f6f747381
        // d82a5823001220fb16f5083412ef1371d031ed4aa239903d84efdadf1ba3
        // cd678e6475b1a232f86776657273696f6e01511220fb16f5083412ef1371
        // d031ed4aa239903d84efdadf1ba3cd678e6475b1a232f8122d0a221220d9
        // c0d5376d26f1931f7ad52d7acc00fc1090d2edb0808bf61eeb0a152826f6
        // 261204f09f8da418a40185011220d9c0d5376d26f1931f7ad52d7acc00fc
        // 1090d2edb0808bf61eeb0a152826f62612310a221220d745b7757f5b4593
        // eeab7820306c7bc64eb496a7410a0d07df7a34ffec4b97f1120962617272
        // 656c657965183a122e0a2401551220a2e1c40da1ae335d4dffe729eb4d5c
        // a23b74b9e51fc535f4a804a261080c294d1204f09f90a11807581220d745
        // b7757f5b4593eeab7820306c7bc64eb496a7410a0d07df7a34ffec4b97f1
        // 12340a2401551220b474a99a2705e23cf905a484ec6d14ef58b56bbe62e9
        // 292783466ec363b5072d120a666973686d6f6e67657218042801551220b4
        // 74a99a2705e23cf905a484ec6d14ef58b56bbe62e9292783466ec363b507
        // 2d666973682b01551220a2e1c40da1ae335d4dffe729eb4d5ca23b74b9e5
        // 1fc535f4a804a261080c294d6c6f62737465720100000028000000c80000
        // 0000000000a2e1c40da1ae335d4dffe729eb4d5ca23b74b9e51fc535f4a8
        // 04a261080c294d9401000000000000b474a99a2705e23cf905a484ec6d14
        // ef58b56bbe62e9292783466ec363b5072d6b01000000000000d745b7757f
        // 5b4593eeab7820306c7bc64eb496a7410a0d07df7a34ffec4b97f1120100
        // 0000000000d9c0d5376d26f1931f7ad52d7acc00fc1090d2edb0808bf61e
        // eb0a152826f6268b00000000000000fb16f5083412ef1371d031ed4aa239
        // 903d84efdadf1ba3cd678e6475b1a232f83900000000000000
        let car_filepath = "./tests/spec_fixtures/carv2-basic.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
        decode_car(&mut file, true).await.unwrap();
    }
}
