use cid::Cid;
use futures::StreamExt;
use futures::{io::Cursor, AsyncRead};
use ipfs_unixfs::file::adder::FileAdder;
use ipfs_unixfs::pb::FlatUnixFs;
use rs_car::CarReader;
use std::io::{BufRead, BufReader, Read, Write};

fn main() {
    futures::executor::block_on(async {
        let file_path = "file_1000000.car";
        let mut file = std::fs::File::open(file_path).unwrap();

        let file_path_out = format!("{file_path}.out");
        let mut file_out = std::fs::File::create(file_path_out).unwrap();

        // Read entire file in memory
        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data).unwrap();

        let mut input_stream = Cursor::new(file_data);

        write_car(&mut input_stream, &mut file_out).await;
    })
}

async fn write_car<R: AsyncRead + Send + Unpin, W: Write>(car_read: &mut R, file_out: &mut W) {
    let mut streamer = CarReader::new(car_read, true).await.unwrap();

    let root_cid = streamer.header.roots[0];

    let mut links_sorted: Vec<Cid> = vec![root_cid];
    let mut links_sorted_ptr: usize = 0;

    while let Some(item) = streamer.next().await {
        let (cid, block) = item.unwrap();

        let inner = FlatUnixFs::try_from(block.as_slice()).unwrap();

        if inner.links.len() == 0 {
            // Leaf data node, expected to be sorted
            // TODO: Cache leaf nodes until finding the first
            if links_sorted.get(links_sorted_ptr) == Some(&cid) {
                links_sorted_ptr += 1;
                // file_data.extend_from_slice(inner.data.Data.unwrap().as_ref());
                file_out
                    .write_all(inner.data.Data.unwrap().as_ref())
                    .unwrap();
            } else {
                panic!("not expected leaf node");
            }
        } else {
            let links_cid = inner
                .links
                .iter()
                .map(|link| hash_to_cid(link.Hash.as_ref().unwrap()))
                .collect::<Vec<Cid>>();

            // Replace for existing link
            let index = links_sorted
                .iter()
                .position(|&r| r == cid)
                .expect("CID not referenced in link before");

            if index < links_sorted_ptr {
                panic!("cannot replace consumed link")
            }

            links_sorted.splice(index..index + 1, links_cid);
        };
    }
}

/// # Example
/// ```
/// compute_blocks_of_file(std::io::stdin());
/// ```
#[allow(dead_code)]
fn compute_blocks_of_file<R: Read>(r: R) {
    // CarDecodeBlockStreamer::new(r, validate_block_hash)

    let mut adder = FileAdder::default();
    let mut input = BufReader::with_capacity(adder.size_hint(), r);

    let blocks = loop {
        match input.fill_buf().unwrap() {
            x if x.is_empty() => {
                eprintln!("finishing");
                eprintln!("{:?}", adder);
                break adder.finish();
            }
            x => {
                let mut total = 0;

                while total < x.len() {
                    let (_blocks, consumed) = adder.push(&x[total..]);
                    total += consumed;
                }

                assert_eq!(total, x.len());
                input.consume(total);
            }
        }
    };

    for block in blocks {
        println!("{:?}", block.0.to_string());
    }
}

fn hash_to_cid(hash: &[u8]) -> Cid {
    Cid::try_from(hash).unwrap()
}
