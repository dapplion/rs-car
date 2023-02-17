use cid::Cid;
use futures::{AsyncRead, AsyncReadExt};
use multihash::MultihashGeneric;
use sha2::{Digest, Sha256};

use crate::{error::CarDecodeError, varint::read_varint_u64};

const CODE_SHA2_256: u64 = 0x12;
const DIGEST_SIZE: usize = 64;

pub(crate) async fn read_block_cid<R: AsyncRead + Unpin>(
    src: &mut R,
) -> Result<Cid, CarDecodeError> {
    let (version, _) = read_varint_u64(src)
        .await?
        .ok_or(cid::Error::InvalidCidVersion)?;
    let (codec, _) = read_varint_u64(src)
        .await?
        .ok_or(cid::Error::InvalidCidV0Codec)?;

    // A CIDv0 is indicated by a first byte of 0x12 followed by 0x20 which specifies a 32-byte (0x20) length SHA2-256 (0x12) digest.
    if [version, codec] == [CODE_SHA2_256, 0x20] {
        let mut digest = [0u8; 32];
        src.read_exact(&mut digest).await?;
        let mh = MultihashGeneric::wrap(version, &digest).expect("Digest is always 32 bytes.");
        return Ok(Cid::new_v0(mh)?);
    }

    // CIDv1 components:
    // 1. Version as an unsigned varint (should be 1)
    // 2. Codec as an unsigned varint (valid according to the multicodec table)
    // 3. The raw bytes of a multihash
    let version = cid::Version::try_from(version).unwrap();
    match version {
        cid::Version::V0 => Err(cid::Error::InvalidExplicitCidV0)?,
        cid::Version::V1 => {
            let mh = read_multihash(src).await?;
            Ok(Cid::new(version, codec, mh)?)
        }
    }
}

async fn read_multihash<R: AsyncRead + Unpin>(
    r: &mut R,
) -> Result<MultihashGeneric<DIGEST_SIZE>, CarDecodeError> {
    let (code, _) = read_varint_u64(r)
        .await?
        .ok_or(CarDecodeError::InvalidMultihash(
            "invalid code varint".to_string(),
        ))?;
    let (size, _) = read_varint_u64(r)
        .await?
        .ok_or(CarDecodeError::InvalidMultihash(
            "invalid size varint".to_string(),
        ))?;

    if size > u8::MAX as u64 {
        panic!("digest size {} > max {}", size, DIGEST_SIZE)
    }

    let mut digest = [0; DIGEST_SIZE];
    r.read_exact(&mut digest[..size as usize]).await?;

    // TODO: Sad, copies the digest (again)..
    // Multihash does not expose a way to construct Self without some decoding or copying
    // unwrap: multihash must be valid since it's constructed manually
    Ok(MultihashGeneric::wrap(code, &digest[..size as usize]).unwrap())
}

pub(crate) fn assert_block_cid(cid: &Cid, block: &[u8]) -> Result<(), CarDecodeError> {
    match cid.hash().code() {
        CODE_SHA2_256 => {
            let mut hasher = Sha256::new();
            hasher.update(block);
            let block_digest = hasher.finalize();
            let cid_digest = cid.hash().digest();

            // TODO: Remove need to copy on .to_vec()
            if cid_digest != block_digest.to_vec() {
                return Err(CarDecodeError::BlockDigestMismatch(format!(
                    "sha2-256 digest mismatch cid {:?} cid digest {} block digest {}",
                    cid,
                    hex::encode(cid_digest),
                    hex::encode(block_digest)
                )));
            }
        }
        code => return Err(CarDecodeError::UnsupportedHashCode { code }),
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{assert_block_cid, read_block_cid, read_multihash};
    use crate::{cid::CODE_SHA2_256, error::CarDecodeError};
    use cid::Cid;
    use futures::executor;
    use futures::io::Cursor;
    use multihash::{Multihash, MultihashGeneric};

    const CID_V0_STR: &str = "QmUU2HcUBVSXkfWPUc3WUSeCMrWWeEJTuAgR9uyWBhh9Nf";
    const CID_V0_HEX: &str = "12205b0995ced69229d26009c53c185a62ea805a339383521edbed1028c496615448";
    const CID_DIGEST: &str = "5b0995ced69229d26009c53c185a62ea805a339383521edbed1028c4966154480000000000000000000000000000000000000000000000000000000000000000";

    const CID_V1_STR: &str = "bafyreihyrpefhacm6kkp4ql6j6udakdit7g3dmkzfriqfykhjw6cad5lrm";
    const CID_V1_HEX: &str =
        "01711220f88bc853804cf294fe417e4fa83028689fcdb1b1592c5102e1474dbc200fab8b";

    // Cursor = easy way to get AsyncRead from an AsRef<[u8]>
    fn from_hex(input: &str) -> Cursor<Vec<u8>> {
        Cursor::new(hex::decode(input).unwrap())
    }

    #[test]
    fn read_block_cid_from_v0() {
        let cid_expected = Cid::try_from(CID_V0_STR).unwrap();

        let mut input_stream = from_hex(CID_V0_HEX);
        let cid = executor::block_on(read_block_cid(&mut input_stream)).unwrap();

        assert_eq!(cid, cid_expected);
    }

    #[test]
    fn read_multihash_from_v0() {
        let digest = hex::decode(CID_DIGEST).unwrap();
        let mh_expected = MultihashGeneric::<64>::wrap(CODE_SHA2_256, &digest).unwrap();

        let mut input_stream = from_hex(CID_V0_HEX);
        let mh = executor::block_on(read_multihash(&mut input_stream)).unwrap();

        assert_eq!(mh, mh_expected);

        // Sanity check, same result as sync version. Sync API can dynamically shrink size to 32 bytes
        let mh_sync = Multihash::read(&mut mh_expected.to_bytes().as_slice()).unwrap();
        assert_eq!(mh_sync, mh_expected);
    }

    #[test]
    fn read_block_cid_from_v1() {
        let cid_expected = Cid::try_from(CID_V1_STR).unwrap();

        let mut input_stream = from_hex(CID_V1_HEX);
        let cid = executor::block_on(read_block_cid(&mut input_stream)).unwrap();

        // Double check multihash before full CID
        assert_eq!(cid.hash(), cid_expected.hash());

        assert_eq!(cid, cid_expected);
    }

    #[test]
    fn read_multihash_error_varint_unexpected_eof() {
        let mut input_stream = from_hex("ffff");

        match executor::block_on(read_multihash(&mut input_stream)) {
            Err(CarDecodeError::IoError(err)) => {
                assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof)
            }
            x => panic!("other result {:?}", x),
        }
    }

    #[test]
    fn assert_block_cid_v0_helloworld() {
        // simple dag-pb of string "helloworld"
        let cid = Cid::try_from("QmUU2HcUBVSXkfWPUc3WUSeCMrWWeEJTuAgR9uyWBhh9Nf").unwrap();
        let block = hex::decode("0a110802120b68656c6c6f776f726c640a180b").unwrap();
        assert_block_cid(&cid, &block).unwrap();
    }
}
