use asynchronous_codec::{Decoder, Encoder};
use bytes::{BufMut, Bytes, BytesMut};
use cid::Cid;
use libipld;
use multibase;
use std::{
    fmt,
    hash::{Hash, Hasher},
    io, mem,
};
use unsigned_varint::{codec, encode};

pub struct CARv1Codec {
    varint_decoder: codec::Uvi<u64>,
    decoder_state: CodecDecodeState,
}

struct CARv1Header {
    version: u8,
    // roots: [&Any]
}

#[derive(Debug, Clone)]
enum CodecDecodeState {
    HeaderVarint,
    HeaderBlock { header_len: usize },
    DataVarint,
    DataCID { data_len: usize },
}

impl CARv1Codec {
    pub fn new() -> CARv1Codec {
        CARv1Codec {
            varint_decoder: codec::Uvi::default(),
            decoder_state: CodecDecodeState::HeaderVarint,
        }
    }
}

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
impl Decoder for CARv1Codec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // CARv1 prefixed with unsigned varint = num of bytes beyond varint of Header block

        // TODO: Decode Header block as DAG-CBOR (tag 42 CIDs)
        // - assert header.version == 1
        // - assert header.roots.len > 0

        // Read data starting at position = header_varint_len + header_len
        // Data section is the concatenation of one or more IPLD blocks as
        // `[varint|CID|block]`
        // - usigned varint of the length of CID + data block parts
        // - CID of the block, encoded as raw byte
        // - binary data of the block

        loop {
            println!("step {:?}", self.decoder_state);

            match self.decoder_state {
                CodecDecodeState::HeaderVarint => match self.varint_decoder.decode(src)? {
                    Some(header_len) => {
                        println!("read header len {}", header_len);
                        self.decoder_state = CodecDecodeState::HeaderBlock {
                            header_len: header_len as usize,
                        };
                    }
                    None => return Ok(None),
                },

                CodecDecodeState::HeaderBlock { header_len } => {
                    if src.len() < header_len {
                        let to_reserve = header_len - src.len();
                        src.reserve(to_reserve);
                        return Ok(None);
                    }

                    let header_buf = src.split_to(header_len);
                    println!("read header block {:?}", header_buf);

                    // TODO: Decode header
                    // libipld::cbor::decode::read_bytes(r, len);

                    self.decoder_state = CodecDecodeState::DataVarint;
                }

                CodecDecodeState::DataVarint => match self.varint_decoder.decode(src)? {
                    Some(data_len) => {
                        println!("read data varint {}", data_len);
                        self.decoder_state = CodecDecodeState::DataCID {
                            data_len: data_len as usize,
                        }
                    }
                    None => return Ok(None),
                },

                CodecDecodeState::DataCID { data_len } => {
                    if src.len() < data_len {
                        let to_reserve = data_len - src.len();
                        src.reserve(to_reserve);
                        return Ok(None);
                    }

                    let mut cid_block_buf = src.split_to(data_len);
                    let cid_block_buf_vec = cid_block_buf.to_vec();

                    println!("cid_block_buf_vec {:?}", cid_block_buf_vec);

                    let cid = Cid::try_from(cid_block_buf_vec)
                        .map_err(|err| {
                            // Debug
                            if src.get(0) == Some(&0x12) && src.get(1) == Some(&0x20) {
                                // CIDv0
                                println!("CIDv0");
                            } else {
                                // CIDv1 components:
                                // 1. Version as an unsigned varint (should be 1)
                                // 2. Codec as an unsigned varint (valid according to the multicodec table)
                                // 3. The raw bytes of a multihash
                                let version = self
                                    .varint_decoder
                                    .decode(src)
                                    .unwrap()
                                    .expect("no version in CID");
                                let codec = self
                                    .varint_decoder
                                    .decode(src)
                                    .unwrap()
                                    .expect("no codec in CID");
                                println!("CIDv1 {} {}", version, codec)
                            }

                            err
                        })
                        .unwrap();

                    let cid_len = cid.encoded_len();
                    println!("cid_len {}", cid_len);

                    // println!("decoded CID {:?}", cid.to_string());

                    // A CIDv0 is indicated by a first byte of 0x12 followed by 0x20 which specifies a 32-byte (0x20) length SHA2-256 (0x12) digest.
                    // let byte0 = src.get(0);
                    // if src.get(0) == Some(&0x12) && src.get(1) == Some(&0x20) {
                    //     // CIDv0
                    //     println!("CIDv0");
                    // } else {
                    //     // CIDv1 components:
                    //     // 1. Version as an unsigned varint (should be 1)
                    //     // 2. Codec as an unsigned varint (valid according to the multicodec table)
                    //     // 3. The raw bytes of a multihash
                    //     let version = self.varint_decoder.decode(src)?.expect("no version in CID");
                    //     let codec = self.varint_decoder.decode(src)?.expect("no codec in CID");
                    //     println!("CIDv1 {} {}", version, codec)
                    // }

                    return Ok(Some(cid_block_buf.split_off(cid_len)));
                }
            }
        }
    }

    // fn todo() {
    //     let num = header >> 3;
    //     let out = match header & 7 {
    //         0 => Frame::Open {
    //             stream_id: RemoteStreamId::dialer(num),
    //         },
    //         1 => Frame::Data {
    //             stream_id: RemoteStreamId::listener(num),
    //             data: buf.freeze(),
    //         },
    //         2 => Frame::Data {
    //             stream_id: RemoteStreamId::dialer(num),
    //             data: buf.freeze(),
    //         },
    //         3 => Frame::Close {
    //             stream_id: RemoteStreamId::listener(num),
    //         },
    //         4 => Frame::Close {
    //             stream_id: RemoteStreamId::dialer(num),
    //         },
    //         5 => Frame::Reset {
    //             stream_id: RemoteStreamId::listener(num),
    //         },
    //         6 => Frame::Reset {
    //             stream_id: RemoteStreamId::dialer(num),
    //         },
    //         _ => {
    //             let msg = format!("Invalid mplex header value 0x{header:x}");
    //             return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
    //         }
    //     };
    // }
}

fn decode_cid(buf: BytesMut) {}
