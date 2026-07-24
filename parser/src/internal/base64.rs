//! A minimal, dependency-free base64 encoder.
//!
//! This crate embeds referenced images as `data:` URIs when the `data-uri`
//! document attribute is set (see
//! [`image_uri`](crate::parser::InlineSubstitutionRenderer::image_uri)), which
//! requires base64-encoding the image bytes. Rather than pull in an external
//! crate for what is a small, well-specified transform, the standard-alphabet
//! encoder lives here.
//!
//! The output matches Ruby's `[bytes].pack('m0')` (equivalently
//! `Base64.strict_encode64`): the standard alphabet, `=` padding, and no line
//! breaks – exactly what a `data:` URI needs.

/// The standard base64 alphabet (RFC 4648), indexed by 6-bit value.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `input` as base64 using the standard alphabet, with `=` padding and
/// no line breaks.
///
/// This mirrors Ruby's `[input].pack('m0')` (a.k.a. `Base64.strict_encode64`),
/// the form Asciidoctor uses when building a `data:` URI.
pub(crate) fn strict_encode(input: &[u8]) -> String {
    // Every 3 input bytes become 4 output characters, rounding up.
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    let mut chunks = input.chunks_exact(3);

    for chunk in chunks.by_ref() {
        // `chunks_exact(3)` yields exactly three bytes per chunk.
        let &[a, b, c] = chunk else { continue };
        let n = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
        out.push(sextet(n >> 18));
        out.push(sextet(n >> 12));
        out.push(sextet(n >> 6));
        out.push(sextet(n));
    }

    // The final 1 or 2 bytes are padded out to a full 4-character group with
    // `=`.
    match chunks.remainder() {
        [a] => {
            let n = u32::from(*a) << 16;
            out.push(sextet(n >> 18));
            out.push(sextet(n >> 12));
            out.push('=');
            out.push('=');
        }
        [a, b] => {
            let n = (u32::from(*a) << 16) | (u32::from(*b) << 8);
            out.push(sextet(n >> 18));
            out.push(sextet(n >> 12));
            out.push(sextet(n >> 6));
            out.push('=');
        }
        _ => {}
    }

    out
}

/// Maps the low 6 bits of `value` to its base64 alphabet character.
fn sextet(value: u32) -> char {
    // Masking to 6 bits keeps the index within `0..64`, so the alphabet lookup
    // can never go out of bounds.
    #[allow(clippy::indexing_slicing)]
    let c = ALPHABET[(value & 0x3f) as usize];

    c as char
}

#[cfg(test)]
mod tests {
    use super::strict_encode;

    #[test]
    fn rfc4648_test_vectors() {
        // The canonical vectors from RFC 4648 §10 exercise every padding case.
        assert_eq!(strict_encode(b""), "");
        assert_eq!(strict_encode(b"f"), "Zg==");
        assert_eq!(strict_encode(b"fo"), "Zm8=");
        assert_eq!(strict_encode(b"foo"), "Zm9v");
        assert_eq!(strict_encode(b"foob"), "Zm9vYg==");
        assert_eq!(strict_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(strict_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encodes_full_byte_range() {
        // A run of bytes spanning the high bit confirms the `+` and `/` alphabet
        // slots and that no line breaks are emitted.
        let bytes: Vec<u8> = (0u8..=255).collect();
        let encoded = strict_encode(&bytes);

        assert!(!encoded.contains('\n'));
        assert!(encoded.starts_with("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8g"));
        assert!(encoded.contains('+'));
        assert!(encoded.contains('/'));

        // 256 bytes → 342 base64 chars (256 / 3 = 85 remainder 1 → 86 groups).
        assert_eq!(encoded.len(), 344);
    }
}
