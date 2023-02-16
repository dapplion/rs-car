use asynchronous_codec::Framed;
use std::io::{BufRead, BufReader, Read};
use unsigned_varint;
use varint;

mod async_fn;
mod codec;

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
pub fn read_carv1<R: Read>(buf_reader: BufReader<R>) {}

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

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

/// ReadVersion reads the version from the pragma.
/// This function accepts both CARv1 and CARv2 payloads.
///
/// ```go
/// func ReadVersion(r io.Reader, opts ...Option) (uint64, error) {
/// 	o := ApplyOptions(opts...)
/// 	header, err := carv1.ReadHeader(r, o.MaxAllowedHeaderSize)
/// 	if err != nil {
/// 		return 0, err
/// 	}
/// 	return header.Version, nil
/// }
/// ```
fn read_version() {}

#[cfg(test)]
mod tests {
    use asynchronous_codec::FramedRead;

    use crate::async_fn::decode_carv1_header;
    use crate::codec::CARv1Codec;

    use super::*;
    use futures::{Stream, StreamExt};
    use std::fs;
    use std::io::prelude::*;
    use std::io::BufReader;

    #[tokio::test]
    async fn open_car_v1_framed() {
        let car_filepath = "./testdata/helloworld.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
        let mut file_framed = FramedRead::new(file, CARv1Codec::new());

        // Trigger reading all stream
        loop {
            match file_framed.next().await {
                Some(value) => {
                    // Process the value here
                    println!("read stream value: {:?}", value.unwrap());
                }
                None => {
                    // End of stream
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn open_car_v1_async() {
        let car_filepath = "./testdata/helloworld.car";
        let mut file = async_std::fs::File::open(car_filepath).await.unwrap();

        let header = decode_carv1_header(&mut file).await.unwrap();
        println!("{:?}", header);
    }
}
