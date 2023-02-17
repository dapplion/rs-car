use cid::Cid;
use futures::io::Cursor;
use futures::AsyncRead;
use futures::AsyncReadExt;
use std::io;

use crate::carv1_header::decode_carv1_header;
use crate::carv1_header::CarV1Header;
use crate::carv2_header::decode_carv2_header;
use crate::carv2_header::CarV2Header;
use crate::carv2_header::CARV2_HEADER_SIZE;
use crate::carv2_header::CARV2_PRAGMA_SIZE;
use crate::cid::{assert_block_cid, read_block_cid};
use crate::error::CarDecodeError;
use crate::varint::read_varint_u64;

pub async fn decode_car<R: AsyncRead + Unpin>(r: &mut R) -> Result<(), CarDecodeError> {
    let header = read_car_header(r).await?;
    println!("header {:?}", header);

    // When to stop streaming blocks?
    // - CARv1: on EOF matching end of block
    // - CARv2: after reading data_size of CARv1 payload bytes

    let mut read_bytes = 0;

    loop {
        // TODO: Should only break on EOF at the very begining of the block
        let (cid, block, block_len) = match decode_block(r).await {
            Ok(res) => res,
            Err(CarDecodeError::BlockStartEOF) if header.eof_stream == StreamEnd::OnBlockEOF => {
                break
            }
            Err(err) => return Err(err),
        };

        println!("block {:?} read_bytes {}", cid, read_bytes);

        read_bytes += block_len;
        if let StreamEnd::AfterNBytes(blocks_len) = header.eof_stream {
            if read_bytes >= blocks_len {
                break;
            }
        }

        // TODO: Should this be done always? And here?
        assert_block_cid(&cid, &block)?;
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
enum StreamEnd {
    AfterNBytes(usize),
    OnBlockEOF,
}

#[derive(Debug)]
struct CarHeader {
    header_v1: CarV1Header,
    header_v2: Option<CarV2Header>,
    eof_stream: StreamEnd,
}

async fn read_car_header<R: AsyncRead + Unpin>(r: &mut R) -> Result<CarHeader, CarDecodeError> {
    let (header, _) = read_carv1_header(r).await?;
    println!("header start {:?}", header);

    match header.version {
        1 => {
            return Ok(CarHeader {
                header_v1: header,
                header_v2: None,
                eof_stream: StreamEnd::OnBlockEOF,
            })
        }
        2 => {
            let (header_v2, (header_v1, header_v1_len)) = read_carv2_header(r).await?;
            let blocks_len = header_v2.data_size as usize - header_v1_len;
            println!("carv2 header {:?} {:?}", header_v2, header_v1);
            return Ok(CarHeader {
                header_v1,
                header_v2: Some(header_v2),
                eof_stream: StreamEnd::AfterNBytes(blocks_len),
            });
        }
        _ => {
            return Err(CarDecodeError::UnsupportedCarVersion {
                version: header.version,
            })
        }
    }
}

/// # Returns
///
/// (header, total header byte length including varint)
async fn read_carv1_header<R: AsyncRead + Unpin>(
    src: &mut R,
) -> Result<(CarV1Header, usize), CarDecodeError> {
    // Decode header varint
    let (header_len, varint_len) =
        read_varint_u64(src)
            .await?
            .ok_or(CarDecodeError::InvalidCarV1Header(
                "invalid header varint".to_string(),
            ))?;

    let mut header_buf = vec![0u8; header_len as usize];
    src.read_exact(&mut header_buf).await?;

    let header = decode_carv1_header(&header_buf)?;

    Ok((header, header_len as usize + varint_len))
}

async fn read_carv2_header<R: AsyncRead + Unpin>(
    r: &mut R,
) -> Result<(CarV2Header, (CarV1Header, usize)), CarDecodeError> {
    let mut header_buf = [0u8; CARV2_HEADER_SIZE];
    r.read_exact(&mut header_buf).await?;

    let header_v2 = decode_carv2_header(&header_buf)?;

    // Read padding, and throw away
    let padding_len = header_v2.data_offset as usize - CARV2_PRAGMA_SIZE - CARV2_HEADER_SIZE;
    if padding_len > 0 {
        let mut padding_buf = vec![0u8; padding_len as usize];
        r.read_exact(&mut padding_buf).await?;
    }

    // Read inner CARv1 header
    let header_v1 = read_carv1_header(r).await?;

    Ok((header_v2, header_v1))
}

/// # Returns
///
/// (cid, block buffer, total block byte length including varint)
async fn decode_block<R: AsyncRead + Unpin>(
    r: &mut R,
) -> Result<(Cid, Vec<u8>, usize), CarDecodeError> {
    let (len, cid, varint_len) = decode_block_header(r).await?;

    let block_len = len - cid.encoded_len();

    let mut block_buf = vec![0u8; block_len];
    r.read_exact(&mut block_buf).await?;

    Ok((cid, block_buf, len + varint_len))
}

async fn decode_block_header<R: AsyncRead + Unpin>(
    src: &mut R,
) -> Result<(usize, Cid, usize), CarDecodeError> {
    let (len, varint_len) = match read_varint_u64(src).await {
        Ok(Some(len)) => len,
        Ok(None) => {
            return Err(CarDecodeError::InvalidBlockHeader(
                "invalid block header varint".to_string(),
            ))
        }
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(CarDecodeError::BlockStartEOF)
        }
        Err(err) => Err(err)?,
    };

    if len == 0 {
        return Err(CarDecodeError::InvalidBlockHeader(
            "zero length".to_string(),
        ));
    }

    let cid = read_block_cid(src).await?;

    Ok((len as usize, cid, varint_len))
}

#[cfg(test)]
mod tests {
    use super::read_carv1_header;
    use crate::{
        carv1_header::CarV1Header,
        carv2_header::{CARV2_PRAGMA, CARV2_PRAGMA_SIZE},
    };
    use futures::io::Cursor;

    #[tokio::test]
    async fn read_carv1_header_v2_pragma() {
        assert_eq!(
            read_carv1_header(&mut Cursor::new(&CARV2_PRAGMA))
                .await
                .unwrap(),
            (
                CarV1Header {
                    version: 2,
                    roots: None
                },
                CARV2_PRAGMA_SIZE
            )
        )
    }
}
