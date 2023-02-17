use rs_car::decode_car;
use std::fs::{self, DirEntry};

const FIXTURES_DIRPATH: &str = "tests/fixtures";

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

        match decode_car(&mut file).await {
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
