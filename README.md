# rs-car

Rust implementation of the [CAR specifications](https://ipld.io/specs/transport/car/), both [CARv1](https://ipld.io/specs/transport/car/carv1/) and [CARv2](https://ipld.io/specs/transport/car/carv2/).

## Usage

```rs
let mut file = async_std::fs::File::open(car_filepath).await.unwrap();
let block_stream = decode_car_async_stream(&mut file, true).await.unwrap();

while let Some(item) = block_stream.next().await {
    let (cid, block) = item.unwrap();
    // Do something with CAR block
}
```
