#![no_std]
#![forbid(unsafe_code)]

#![doc = include_str!("../README.md")]

extern crate no_std_compat as std;

use std::prelude::v1::*;
use std::collections::VecDeque;

use memchr::memchr;

fn ceil_char_boundary_offset<I: Iterator<Item = u8>>(mut src: I) -> usize {
    for i in 0..4 {
        match src.next() {
            None => {
                debug_assert_eq!(i, 0);
                return 0;
            }
            Some(b) => if (b as i8) >= -0x40 { // equivalent to: b < 128 || b >= 192 (copied from u8::is_utf8_char_boundary)
                return i;
            }
        }
    }
    unreachable!()
}
#[test]
fn test_ceil_char_boundary_offset() {
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(0)), 0);
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(1)), 0);
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(2)), 0);
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(3)), 0);
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(4)), 0);
    assert_eq!(ceil_char_boundary_offset("hello".as_bytes().iter().copied().skip(5)), 0);

    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(0)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(1)), 2);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(2)), 1);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(3)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(4)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(5)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(6)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(7)), 2);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(8)), 1);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(9)), 0);
    assert_eq!(ceil_char_boundary_offset("한eel들o".as_bytes().iter().copied().skip(10)), 0);
}

/// The level of precision used for removing old content from a [`StringRing`].
pub enum Granularity {
    /// Remove as few characters as possible (may result in partial lines at the beginning).
    Character,
    /// Remove entire lines at a time, but always as few lines as possible.
    Line,
}

/// A circular string buffer with a set maximum size.
/// 
/// Strings can be pushed onto the end of the buffer.
/// If the maximum size would be exceeded, the oldest content will be removed to make space.
/// The specific strategy for removing old content is defined by [`Granularity`].
pub struct StringRing {
    content: VecDeque<u8>,
    max_size: usize,
    granularity: Granularity,
    discarding: bool,
}
impl StringRing {
    /// Creates a new, empty [`StringRing`] with the given settings.
    pub fn new(max_size: usize, granularity: Granularity) -> Self {
        Self {
            granularity, max_size,
            content: Default::default(),
            discarding: false,
        }
    }

    /// Gets the current size of the stored string.
    pub fn len(&self) -> usize {
        self.content.len()
    }
    /// Checks if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
    /// Removes all content from the buffer and resets it to the initial state.
    pub fn clear(&mut self) {
        self.content.clear();
        self.discarding = false;
    }

    /// Gets a reference to the raw stored data.
    /// Note that because this is a circular buffer of bytes, the content may be in two distinct pieces which may not independently be valid UTF-8 sequences.
    /// However, the two slices together (in the returned order) always represents a valid UTF-8 string.
    pub fn as_slices(&self) -> (&[u8], &[u8]) {
        self.content.as_slices()
    }
    /// Makes the content of the circular buffer contiguous and returns it as a mutable slice.
    pub fn make_contiguous(&mut self) -> &mut str {
        std::str::from_utf8_mut(self.content.make_contiguous()).unwrap()
    }

    /// Appends content to the end of the buffer.
    /// If this would cause the buffer to exceed its maximum size, old content is removed to make room (as defined by [`Granularity`]).
    /// 
    /// It is guaranteed that pushing `a` followed by `b` is equivalent to pushing `a + b`.
    /// In [`Granularity::Line`] mode, this may lead to unexpected results if `a` does not end in a new line,
    /// as bytes may need to be discarded from the beginning of `b` to reach the end of a line being removed.
    /// This information is stored internally as a simple state machine to ensure stream consistency.
    pub fn push(&mut self, mut s: &str) {
        loop { // performs at most 3 iterations
            if self.discarding {
                debug_assert!(self.content.is_empty());
                match self.granularity {
                    Granularity::Character => unreachable!(),
                    Granularity::Line => match memchr(b'\n', s.as_bytes()) {
                        Some(x) => {
                            self.discarding = false;
                            s = &s[x + 1..];
                        }
                        None => return,
                    }
                }
            }
            debug_assert!(!self.discarding);

            let quota = (self.content.len() + s.len()).saturating_sub(self.max_size);
            if quota == 0 {
                self.content.extend(s.as_bytes().iter().copied());
                return;
            }

            if !self.content.is_empty() {
                if quota >= self.content.len() {
                    match self.granularity {
                        Granularity::Character => (),
                        Granularity::Line => self.discarding = *self.content.back().unwrap() != b'\n',
                    }
                    self.content.clear();
                } else {
                    let (a, b) = self.content.as_slices();
                    let delta = ceil_char_boundary_offset(a.iter().chain(b).copied().skip(quota));
                    let last_removed = self.content[quota + delta - 1];
                    self.content.drain(..quota + delta);
                    match self.granularity {
                        Granularity::Character => (),
                        Granularity::Line => if last_removed != b'\n' {
                            let (a, b) = self.content.as_slices();
                            match memchr(b'\n', a) {
                                Some(x) => { self.content.drain(..x + 1); }
                                None => match memchr(b'\n', b) {
                                    Some(x) => { self.content.drain(..a.len() + x + 1); }
                                    None => {
                                        self.discarding = true;
                                        self.content.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                let delta = ceil_char_boundary_offset(s.as_bytes()[quota..].iter().copied());
                let last_removed = s.as_bytes()[quota + delta - 1];
                s = &s[quota + delta..];
                match self.granularity {
                    Granularity::Character => (),
                    Granularity::Line => self.discarding = last_removed != b'\n',
                }
            }
        }
    }
}
#[test]
fn test_string_ring_char() {
    let mut buf = StringRing::new(18, Granularity::Character);
    assert_eq!(buf.make_contiguous(), "");
    buf.push("hello world");
    assert_eq!(buf.make_contiguous(), "hello world");
    buf.push("this is a test");
    assert_eq!(buf.make_contiguous(), "orldthis is a test");
    buf.push("this is a really long string that will go over the buffer limit");
    assert_eq!(buf.make_contiguous(), "r the buffer limit");
}
#[test]
fn test_string_ring_line() {
    let mut buf = StringRing::new(21, Granularity::Line);
    assert_eq!(buf.make_contiguous(), "");
    buf.push("hello world\n");
    assert_eq!(buf.make_contiguous(), "hello world\n");
    buf.push("this is a test\n");
    assert_eq!(buf.make_contiguous(), "this is a test\n");
    buf.push("small\n");
    assert_eq!(buf.make_contiguous(), "this is a test\nsmall\n");
    buf.push("a\n");
    assert_eq!(buf.make_contiguous(), "small\na\n");
    buf.push("this is a really long line that will go over the limit\n");
    assert_eq!(buf.make_contiguous(), "");
    buf.push("another test");
    assert_eq!(buf.make_contiguous(), "another test");
    buf.push("bananasyo");
    assert_eq!(buf.make_contiguous(), "another testbananasyo");
    buf.push("x");
    assert_eq!(buf.make_contiguous(), "");
    buf.push("more content");
    assert_eq!(buf.make_contiguous(), "");
    buf.push("even more content\nand a new line\n");
    assert_eq!(buf.make_contiguous(), "and a new line\n");
}
