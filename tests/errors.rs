use futures::io::Cursor;
use rs_car::decode_car;

enum TestResult {
    Error(&'static str),
    Panic,
    Success(&'static str),
}

macro_rules! load_file_test {
    ($name:ident, $car_hex:expr, $expected:expr) => {
        #[tokio::test]
        async fn $name() {
            let result = std::panic::catch_unwind(|| {
                let mut input = Cursor::new(hex::decode($car_hex).unwrap());
                futures::executor::block_on(decode_car(&mut input))
            });

            match result {
                Ok(Ok(_)) => match $expected {
                    TestResult::Success(_) => {} // Ok
                    TestResult::Error(err) => panic!("expected error but got success: {:?}", err),
                    TestResult::Panic => panic!("expected panic but got success"),
                },
                Ok(Err(err)) => match $expected {
                    TestResult::Success(_) => panic!("expected success but got error: {:?}", err),
                    TestResult::Error(expected_err) => assert_eq!(err.to_string(), expected_err),
                    TestResult::Panic => panic!("expected panic but got error: {:?}", err),
                },
                Err(panic_error) => match $expected {
                    TestResult::Success(_) => panic!("expected panic but got success"),
                    TestResult::Error(expected_err) => {
                        panic!(
                            "expected error but got panic: {:?} \n {:?}",
                            panic_error, expected_err
                        )
                    }
                    TestResult::Panic => {} // Ok
                },
            };
        }
    };
}

load_file_test!(bad_cid_v0, "3aa265726f6f747381d8305825000130302030303030303030303030303030303030303030303030303030303030303030306776657273696f6e010130", TestResult::Error("expected 1 as the cid version number, got: 48"));
load_file_test!(
    bad_header_length,
    "e0e0e0e0a7060c6f6c4cca943c236f4b196723489608edb42a8b8fa80b6776657273696f6e19",
    TestResult::Error("invalid header data, length of read beyond allowable maximum")
);
load_file_test!(bad_section_length, "11a265726f6f7473806776657273696f6e01e0e0e0e0a7060155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca00000000000000000000", TestResult::Error("invalid section data, length of read beyond allowable maximum"));
load_file_test!(bad_section_length2, "3aa265726f6f747381d8305825000130302030303030303030303030303030303030303030303030303030303030303030306776657273696f6e01200130302030303030303030303030303030303030303030303030303030303030303030303030303030303030", TestResult::Error("section length shorter than CID length"));
load_file_test!(
    bad_section_length3,
    "11a265726f6f7473f66776657273696f6e0180",
    TestResult::Error("unexpected EOF")
);
load_file_test!(bad_block_hash_sanity_check, "11a265726f6f7473806776657273696f6e 012e0155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca ffffffffffffffffffff", TestResult::Success(""));

load_file_test!(bad_block_hash, "11a265726f6f7473806776657273696f6e 012e0155122001d448afd928065458cf670b60f5a594d735af0172c8d67f22a81680132681ca ffffffffffffffffffff", TestResult::Error("mismatch in content integrity, expected: bafkreiab2rek7wjiazkfrt3hbnqpljmu24226alszdlh6ivic2abgjubzi, got: bafkreiaaqoxrddiyuy6gxnks6ioqytxhq5a7tchm2mm5htigznwiljukmm"));
load_file_test!(identity_cid, "2f a265726f6f747381d82a581a0001a90200147b226964656e74697479223a22626c6f636b227d6776657273696f6e01 19 01a90200147b226964656e74697479223a22626c6f636b227d", TestResult::Error("mismatch in content integrity, expected: baguqeaaupmrgszdfnz2gs5dzei5ceytmn5rwwit5, got: baguqeaaa"));
