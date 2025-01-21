use futures::io::Cursor;
use rs_car::car_read_all;

enum TestResult {
    Error(&'static str),
    Success,
}

#[derive(PartialEq)]
enum TestOptions {
    None,
    SkipValidateBlockHash,
}

macro_rules! error_test {
    ($name:ident, $car_hex:expr, $expected:expr, $opts:expr) => {
        #[test]
        fn $name() {
            let result = std::panic::catch_unwind(|| {
                let validate_block_hash = match $opts {
                    TestOptions::None => true,
                    TestOptions::SkipValidateBlockHash => false,
                };

                let mut input = Cursor::new(hex::decode($car_hex.replace(" ", "")).unwrap());
                futures::executor::block_on(car_read_all(&mut input, validate_block_hash))
            });

            match result {
                Ok(Ok(_)) => match $expected {
                    TestResult::Success => {} // Ok
                    TestResult::Error(err) => {
                        panic!("expected error but got success: {:?}", err)
                    }
                },
                Ok(Err(err)) => match $expected {
                    TestResult::Success => {
                        panic!("expected success but got error: {:?}", err)
                    }
                    TestResult::Error(expected_err) => {
                        assert_eq!(err.to_string().replace("kown", "known"), expected_err)
                    }
                },
                Err(panic_error) => match $expected {
                    TestResult::Success => panic!("expected panic but got success"),
                    TestResult::Error(expected_err) => {
                        panic!(
                            "expected error but got panic: {:?} \n {:?}",
                            panic_error, expected_err
                        )
                    }
                },
            };
        }
    };
}

error_test!(
    bad_cid_v0,
    "3aa265726f6f747381d8305825000130302030303030303030303030303030303030303030303030303030303030303030306776657273696f6e010130",
    TestResult::Error("InvalidCarV1Header(\"header cbor codec error: DecodeIo(TypeMismatch { name: \\\"CBOR tag\\\", byte: 48 })\")"),
    TestOptions::None
);

error_test!(
    bad_header_length,
    "e0e0e0e0a7060c6f6c4cca943c236f4b196723489608edb42a8b8fa80b6776657273696f6e19",
    TestResult::Error("InvalidCarV1Header(\"header len too big 216830324832\")"),
    TestOptions::None
);

error_test!(
    bad_section_length_1,
    "11a265726f6f7473806776657273696f6e01e0e0e0e0a7060155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca00000000000000000000",
    TestResult::Error("InvalidBlockHeader(\"block len too big 216830324832\")"),
    TestOptions::None
);

error_test!(
    bad_section_length_2,
    "3aa265726f6f747381d8305825000130302030303030303030303030303030303030303030303030303030303030303030306776657273696f6e01200130302030303030303030303030303030303030303030303030303030303030303030303030303030303030",
    TestResult::Error("InvalidCarV1Header(\"header cbor codec error: DecodeIo(TypeMismatch { name: \\\"CBOR tag\\\", byte: 48 })\")"),
    TestOptions::None
);

error_test!(
    bad_section_length_3,
    "11a265726f6f7473f66776657273696f6e0180",
    TestResult::Error("InvalidCarV1Header(\"roots key expected cbor List but got Null\")"),
    TestOptions::None
);

// this should pass because we don't ask the CID be validated even though it doesn't match
error_test!(
    bad_block_hash_skip_verify,
//   header                             cid                                                                          data
    "11a265726f6f7473806776657273696f6e 012e0155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca ffffffffffffffffffff",
    TestResult::Success,
    TestOptions::SkipValidateBlockHash
);

// same as above, but we ask for CID validation
error_test!(
    bad_block_hash_do_verify,
    "11a265726f6f7473806776657273696f6e 012e0155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca ffffffffffffffffffff",
    TestResult::Error("BlockDigestMismatch(\"sha2-256 digest mismatch cid Cid(bafkreiab2rek7wjiazkfrt3hbnqpljmu24226alszdlh6ivic2abgjubzi) cid digest 01d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca block digest 0083af118d18a63c6bb552f21d0c4ee78741f988ecd319d3cd06cb6c85a68a63\")"),
    TestOptions::None
);

// a case where this _could_ be a valid CAR if we allowed identity CIDs and not matching block contents to exist, there's no block bytes in this
error_test!(
    identity_cid,
//   47 {version:1,roots:[identity cid]}                                                               25 identity cid (dag-json {"identity":"block"})
    "2f a265726f6f747381d82a581a0001a90200147b226964656e74697479223a22626c6f636b227d6776657273696f6e01 19 01a90200147b226964656e74697479223a22626c6f636b227d",
    TestResult::Error("BlockDigestMismatch(\"identity digest mismatch cid Cid(baguqeaaupmrgszdfnz2gs5dzei5ceytmn5rwwit5) cid digest 7b226964656e74697479223a22626c6f636b227d block digest \")"),
    TestOptions::None
);
