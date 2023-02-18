use rs_car::decode_car;
use std::fs::{self, DirEntry};

const FIXTURES_DIRPATH: &str = "tests/fixtures";

enum TestResult {
    Error(&'static str),
    Panic,
    Success(&'static str),
}

macro_rules! load_file_test {
    ($name:ident, $file:expr, $expected:expr) => {
        #[tokio::test]
        async fn $name() {
            let result = std::panic::catch_unwind(|| {
                let mut file =
                    futures::executor::block_on(async_std::fs::File::open($file)).unwrap();
                futures::executor::block_on(decode_car(&mut file, true))
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

load_file_test!(
    corrupt_pragma_is_rejected,
    "tests/fixtures/sample-corrupt-pragma.car",
    TestResult::Error("IoError(Kind(UnexpectedEof))")
);
load_file_test!(
    car_v42_is_rejected,
    "tests/fixtures/sample-rootless-v42.car",
    TestResult::Error("UnsupportedCarVersion { version: 42 }")
);
load_file_test!(
    car_v1_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-v1.car",
    TestResult::Error("current header size (61) must match replacement header size (18)")
);
load_file_test!(
    car_v2_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-wrapped-v2.car",
    TestResult::Error("current header size (61) must match replacement header size (18)")
);
// roots:      []cid.Cid{requireDecodedCid(t, "QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n")},
load_file_test!(
    car_v1_non_empty_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-v1.car",
    TestResult::Error("current header size (61) must match replacement header size (57)")
);
// roots:      []cid.Cid{merkledag.NewRawNode([]byte("fish")).Cid()},
load_file_test!(
    car_v1_zero_len_non_empty_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-v1-with-zero-len-section.car",
    TestResult::Error("current header size (61) must match replacement header size (59)")
);
// roots:      []cid.Cid{merkledag.NewRawNode([]byte("fish")).Cid()},
load_file_test!(
    car_v2_non_empty_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-wrapped-v2.car",
    TestResult::Error("current header size (61) must match replacement header size (59)")
);
// roots:      []cid.Cid{merkledag.NewRawNode([]byte("fish")).Cid()},
load_file_test!(
    car_v2_indexless_non_empty_roots_of_different_size_are_not_replaced,
    "tests/fixtures/sample-v2-indexless.car",
    TestResult::Error("current header size (61) must match replacement header size (59)")
);
// roots: []cid.Cid{requireDecodedCid(t, "bafy2bzaced4ueelaegfs5fqu4tzsh6ywbbpfk3cxppupmxfdhbpbhzawfw5od")}
load_file_test!(
    car_v1_same_size_roots_are_replaced,
    "tests/fixtures/sample-v1.car",
    TestResult::Success("")
);
// roots: []cid.Cid{requireDecodedCid(t, "bafy2bzaced4ueelaegfs5fqu4tzsh6ywbbpfk3cxppupmxfdhbpbhzawfw5oi")}
load_file_test!(
    car_v2_same_size_roots_are_replaced,
    "tests/fixtures/sample-wrapped-v2.car",
    TestResult::Success("")
);
// roots: []cid.Cid{requireDecodedCid(t, "bafy2bzaced4ueelaegfs5fqu4tzsh6ywbbpfk3cxppupmxfdhbpbhzawfw5oi")},
load_file_test!(
    car_v2_indexless_same_size_roots_are_replaced,
    "tests/fixtures/sample-v2-indexless.car",
    TestResult::Success("")
);
// roots: []cid.Cid{requireDecodedCid(t, "bafy2bzaced4ueelaegfs5fqu4tzsh6ywbbpfk3cxppupmxfdhbpbhzawfw5o5")},
load_file_test!(
    car_v1_zero_len_same_size_roots_are_replaced,
    "tests/fixtures/sample-v1-with-zero-len-section.car",
    TestResult::Success("")
);

#[tokio::test]
async fn test_fixtures() {
    for file in fs::read_dir(FIXTURES_DIRPATH).unwrap() {
        let file = file.unwrap();

        if !is_car_file(&file) {
            continue;
        }

        let filename = file.path().to_str().unwrap().to_owned();
        let mut file = async_std::fs::File::open(file.path()).await.unwrap();

        println!("Test {}", filename);

        match decode_car(&mut file, true).await {
            Ok(res) => println!("Ok {}: {:?}", filename, res),
            Err(err) => println!("Err {}: {:?}", filename, err),
        }
    }
}

fn is_car_file(file: &DirEntry) -> bool {
    if let Some(ext) = file.path().extension() {
        if !ext.eq("car") {
            return false;
        }
    } else {
        return false;
    }

    if let Ok(ty) = file.file_type() {
        if !ty.is_file() {
            return false;
        }
    } else {
        return false;
    }

    return true;
}
