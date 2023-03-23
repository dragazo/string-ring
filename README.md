In many logging scenarios, it is often useful to limit the logs to some maximum size while keeping only the most recent content.
A common approach is to use a fixed-size circular buffer, where more space can be made for new content by deleting old content.

This crate supports one principle type, [`StringRing`], which is a circular buffer for UTF-8 encoded strings.
Multiple [`Granularity`] modes are supported for various applications, which control how old content is removed.

## Examples

The following is an example of the basic [`Granularity::Character`] mode:

```rust
# use string_ring::*;
// example of a 16-byte circular string buffer
let mut buf = StringRing::new(16, Granularity::Character);
buf.push("hello world!");
assert_eq!(buf.make_contiguous(), "hello world!");
buf.push("more content!");
assert_eq!(buf.make_contiguous(), "ld!more content!");
```

The following is an example of the [`Granularity::Line`] mode, which is often more useful for logging:

```rust
# use string_ring::*;
// example of a 26-byte circular string buffer
let mut buf = StringRing::new(26, Granularity::Line);
buf.push("hello world!\n");
assert_eq!(buf.make_contiguous(), "hello world!\n");
buf.push("more!\n");
assert_eq!(buf.make_contiguous(), "hello world!\nmore!\n");
buf.push("too much!\n");
assert_eq!(buf.make_contiguous(), "more!\ntoo much!\n");
```

## no-std

This crate supports building in `no-std` environments by disabling default features:

```toml
[dependencies]
string-ring = { version = "...", use-default-features = false }
```

Note that the `alloc` crate is still required in this case,
as there is not currently a heapless version of [`StringRing`].
