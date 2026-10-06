//! Minimal SHA-1, used only to identify user-supplied tape and ROM files.

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    // The padding is at most 64 + 8 bytes, so only that is built here: the
    // input itself is hashed where it lies rather than copied first.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let whole = data.len() - data.len() % 64;
    let mut tail = data[whole..].to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in data[..whole].chunks(64).chain(tail.chunks(64)) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (hv, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *hv = hv.wrapping_add(v);
        }
    }

    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

pub fn sha1_hex(data: &[u8]) -> String {
    sha1(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(sha1_hex(long), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    }

    /// Whole blocks are hashed where they lie and only the tail is copied and
    /// padded, so the lengths where that split falls awkwardly are worth
    /// pinning: padding fits in the last block up to 55 bytes and needs a
    /// further one from 56.
    #[test]
    fn block_boundaries() {
        let pattern = |n: usize| -> Vec<u8> { (0..n).map(|i| (i * 7 % 251) as u8).collect() };
        for (n, want) in [
            (0, "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            (1, "5ba93c9db0cff93f52b521d7420e43f6eda2784f"),
            (55, "a83f94113f5292bb7ed9d7df07178ad7d931341a"),
            (56, "372e1b20329e0b2862472089ac00c55505116275"),
            (63, "6944938dc131b6453b7d9637cfcbad8a3db28104"),
            (64, "aec4b7f13a2b75ec13bc0c3f13fa55caf97e621d"),
            (65, "29a20455c2f21fa85c66014ff3b75bfcce5aaba5"),
            (119, "43610a35bbf6783ba62fc82d421abc8b2f3e1abf"),
            (120, "482b555c1bda1cd10f9bdc34081249810af9fd77"),
            (128, "ddde2e96d57b78eda0da1151110a11eaa1d21d7b"),
            (1000, "33f233c97a803d84a0db9f3dbc05b63ff2045d92"),
        ] {
            assert_eq!(sha1_hex(&pattern(n)), want, "length {n}");
        }
    }
}
