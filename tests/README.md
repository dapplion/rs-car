# How to generate test vectors

Run IPFS daemon, exposes by default the [HTTP API](https://docs.ipfs.tech/reference/kubo/rpc/) on port 5001 and a gateway on port 8080.

```
$ ipfs daemon
```

Create a file with some contents, in the example below a UTF8 string of ascending integers concatenated by spaces.

```
$ seq -s ' ' 1 2000 > seq.txt
```

Or a large file filled with random

```
head -c 1M </dev/urandom > rand.bin
```

Add the file to the IPFS node

```
$ ipfs add seq.txt
added QmV3q6mo8oxf2GBuvR7zx7ABFBNP5VrRs3sCr63HQ7kEFC seq.txt
```

Write a CAR stream of the file to disk

```
curl "http://localhost:8080/ipfs/QmV3q6mo8oxf2GBuvR7zx7ABFBNP5VrRs3sCr63HQ7kEFC?format=car" > seq.txt.car
```
