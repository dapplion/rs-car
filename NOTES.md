# Questions

How to handle a future that holds a `&'a mut R`.

```rs
impl<'a, R> Stream for CarDecodeBlockStreamer<'a, R>
where
    R: AsyncRead + Send + Unpin,
{
    type Item = Result<(Cid, Vec<u8>), CarDecodeError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = &mut *self;

        loop {
            match me.decode_header_future.take() {
                Some(mut decode_header_future) => match decode_header_future.poll_unpin(cx) {
                    Poll::Pending => {
                        me.decode_header_future = Some(decode_header_future);
                        return Poll::Pending;
                    }
                    Poll::Ready(_) => {}
                },

                None => {
                    let fut = decode_block(&mut me.r);
                    me.decode_header_future = Some(fut.boxed())
                }
            }
        }
    }
}
```

Compiler error

```
error[E0597]: `self` does not live long enough
   --> src/lib.rs:65:24
    |
65  |         let me = &mut *self;
    |                        ^^^^ borrowed value does not live long enough
...
77  |                 None => me.decode_header_future = Some(decode_block(me.r).boxed()),
    |                                                        -------------------------- cast requires that `self` is borrowed for `'static`
...
114 |     }
    |      - `self` dropped here while still borrowed

error: lifetime may not live long enough
  --> src/lib.rs:77:56
   |
58 | impl<'a, R> Stream for CarDecodeBlockStreamer<'a, R>
   |      -- lifetime `'a` defined here
...
77 |                 None => me.decode_header_future = Some(decode_block(me.r).boxed()),
   |                                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^ cast requires that `'a` must outlive `'static`

error: lifetime may not live long enough
  --> src/lib.rs:77:56
   |
64 |     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
   |                                - let's call the lifetime of this reference `'1`
...
77 |                 None => me.decode_header_future = Some(decode_block(me.r).boxed()),
   |                                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^ cast requires that `'1` must outlive `'static`

error[E0499]: cannot borrow `*me.r` as mutable more than once at a time
  --> src/lib.rs:77:69
   |
77 |                 None => me.decode_header_future = Some(decode_block(me.r).boxed()),
   |                                                        -------------^^^^---------
   |                                                        |            |
   |                                                        |            `*me.r` was mutably borrowed here in the previous iteration of the loop
   |                                                        cast requires that `*me.r` is borrowed for `'static`
```
