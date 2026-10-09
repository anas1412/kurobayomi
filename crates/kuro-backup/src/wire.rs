//! Protobuf wire codec for the `.tachibk` container (area 21, T-21a-001).
//!
//! Hand-written because kotlinx ProtoBuf differs from the usual generators: repeated scalars are unpacked,
//! enums are ordinals, and absent nullable fields must stay absent.

/// Protobuf wire types used by the format. Groups (3, 4) are rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType {
    Varint = 0,
    Fixed64 = 1,
    Len = 2,
    Fixed32 = 5,
}

impl WireType {
    fn from_bits(v: u8) -> Result<Self, WireError> {
        match v {
            0 => Ok(Self::Varint),
            1 => Ok(Self::Fixed64),
            2 => Ok(Self::Len),
            5 => Ok(Self::Fixed32),
            other => Err(WireError::BadWireType(other)),
        }
    }
}

/// Maximum nesting depth of embedded messages. The format nests at most three levels; the limit stops hostile input
/// from exhausting the stack.
const MAX_DEPTH: u32 = 32;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WireError {
    #[error("truncated input")]
    Truncated,
    #[error("varint longer than 10 bytes")]
    VarintTooLong,
    #[error("unsupported wire type {0}")]
    BadWireType(u8),
    #[error("invalid field number 0")]
    FieldZero,
    #[error("field {field} has wire type {got:?}, expected {want:?}")]
    WrongWireType { field: u32, got: WireType, want: WireType },
    #[error("required field {0} missing")]
    MissingField(&'static str),
    #[error("unknown enum ordinal {ordinal} for {name}")]
    BadEnum { name: &'static str, ordinal: u64 },
    #[error("invalid UTF-8 in string field {0}")]
    Utf8(u32),
    #[error("messages nested deeper than {MAX_DEPTH} levels")]
    TooDeep,
    #[error("value out of range for field {field}")]
    OutOfRange { field: u32 },
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

/// Appends protobuf fields to a byte buffer.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    fn varint(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(byte);
                return;
            }
            self.buf.push(byte | 0x80);
        }
    }

    fn tag(&mut self, field: u32, wt: WireType) {
        self.varint(((field as u64) << 3) | wt as u64);
    }

    /// Two's complement as u64: negative values take 10 bytes.
    pub fn int64(&mut self, field: u32, v: i64) {
        self.tag(field, WireType::Varint);
        self.varint(v as u64);
    }

    /// Sign-extended to 64 bits, so negative values take 10 bytes.
    pub fn int32(&mut self, field: u32, v: i32) {
        self.int64(field, v as i64);
    }

    pub fn boolean(&mut self, field: u32, v: bool) {
        self.tag(field, WireType::Varint);
        self.varint(v as u64);
    }

    pub fn enum_ordinal(&mut self, field: u32, v: u8) {
        self.tag(field, WireType::Varint);
        self.varint(v as u64);
    }

    pub fn float32(&mut self, field: u32, v: f32) {
        self.tag(field, WireType::Fixed32);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn float64(&mut self, field: u32, v: f64) {
        self.tag(field, WireType::Fixed64);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn string(&mut self, field: u32, v: &str) {
        self.bytes(field, v.as_bytes());
    }

    pub fn bytes(&mut self, field: u32, v: &[u8]) {
        self.tag(field, WireType::Len);
        self.varint(v.len() as u64);
        self.buf.extend_from_slice(v);
    }

    /// Embedded message, length-prefixed.
    pub fn message<M: Message>(&mut self, field: u32, m: &M) -> Result<(), WireError> {
        let mut inner = Writer::new();
        m.encode(&mut inner)?;
        self.bytes(field, &inner.buf);
        Ok(())
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// Reads protobuf fields. Every read is bounds-checked; no input can panic it.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    depth: u32,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0, depth: 0 }
    }

    fn nested(buf: &'a [u8], depth: u32) -> Self {
        Self { buf, pos: 0, depth }
    }

    /// Next (field number, wire type), or `None` at end of input.
    pub fn next_tag(&mut self) -> Result<Option<(u32, WireType)>, WireError> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }
        let key = self.varint()?;
        let field = key >> 3;
        if field == 0 {
            return Err(WireError::FieldZero);
        }
        let field = u32::try_from(field).map_err(|_| WireError::OutOfRange { field: 0 })?;
        let wt = WireType::from_bits((key & 0x7) as u8)?;
        Ok(Some((field, wt)))
    }

    pub fn varint(&mut self) -> Result<u64, WireError> {
        let mut v: u64 = 0;
        for i in 0..10 {
            let byte = *self.buf.get(self.pos).ok_or(WireError::Truncated)?;
            self.pos += 1;
            v |= ((byte & 0x7f) as u64) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(WireError::VarintTooLong)
    }

    pub fn fixed32(&mut self) -> Result<[u8; 4], WireError> {
        self.take(4).map(|s| s.try_into().unwrap_or([0; 4]))
    }

    pub fn fixed64(&mut self) -> Result<[u8; 8], WireError> {
        self.take(8).map(|s| s.try_into().unwrap_or([0; 8]))
    }

    pub fn len_delimited(&mut self) -> Result<&'a [u8], WireError> {
        let len = self.varint()?;
        let len = usize::try_from(len).map_err(|_| WireError::Truncated)?;
        self.take(len)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::Truncated)?;
        let slice = self.buf.get(self.pos..end).ok_or(WireError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    /// Skips the value of an unknown field.
    pub fn skip(&mut self, wt: WireType) -> Result<(), WireError> {
        match wt {
            WireType::Varint => self.varint().map(|_| ()),
            WireType::Fixed64 => self.fixed64().map(|_| ()),
            WireType::Len => self.len_delimited().map(|_| ()),
            WireType::Fixed32 => self.fixed32().map(|_| ()),
        }
    }

    fn expect(field: u32, got: WireType, want: WireType) -> Result<(), WireError> {
        if got == want { Ok(()) } else { Err(WireError::WrongWireType { field, got, want }) }
    }

    pub fn int64(&mut self, field: u32, wt: WireType) -> Result<i64, WireError> {
        Self::expect(field, wt, WireType::Varint)?;
        Ok(self.varint()? as i64)
    }

    /// Narrowed to `i32`; a value outside the range is an error, not a wrap.
    pub fn int32(&mut self, field: u32, wt: WireType) -> Result<i32, WireError> {
        i32::try_from(self.int64(field, wt)?).map_err(|_| WireError::OutOfRange { field })
    }

    pub fn boolean(&mut self, field: u32, wt: WireType) -> Result<bool, WireError> {
        Ok(self.int64(field, wt)? != 0)
    }

    pub fn enum_ordinal(&mut self, field: u32, wt: WireType) -> Result<u64, WireError> {
        Self::expect(field, wt, WireType::Varint)?;
        self.varint()
    }

    pub fn float32(&mut self, field: u32, wt: WireType) -> Result<f32, WireError> {
        Self::expect(field, wt, WireType::Fixed32)?;
        Ok(f32::from_le_bytes(self.fixed32()?))
    }

    pub fn float64(&mut self, field: u32, wt: WireType) -> Result<f64, WireError> {
        Self::expect(field, wt, WireType::Fixed64)?;
        Ok(f64::from_le_bytes(self.fixed64()?))
    }

    pub fn string(&mut self, field: u32, wt: WireType) -> Result<String, WireError> {
        Self::expect(field, wt, WireType::Len)?;
        let raw = self.len_delimited()?;
        std::str::from_utf8(raw).map(str::to_owned).map_err(|_| WireError::Utf8(field))
    }

    pub fn bytes(&mut self, field: u32, wt: WireType) -> Result<Vec<u8>, WireError> {
        Self::expect(field, wt, WireType::Len)?;
        Ok(self.len_delimited()?.to_vec())
    }

    /// Embedded message. Depth is checked before descending.
    pub fn message<M: Message>(&mut self, field: u32, wt: WireType) -> Result<M, WireError> {
        Self::expect(field, wt, WireType::Len)?;
        if self.depth >= MAX_DEPTH {
            return Err(WireError::TooDeep);
        }
        let raw = self.len_delimited()?;
        M::decode(&mut Reader::nested(raw, self.depth + 1))
    }

    /// Repeated scalar: accepts both the unpacked (one tag per element) and the packed (length-delimited) layout.
    pub fn repeated_int64(&mut self, field: u32, wt: WireType, out: &mut Vec<i64>) -> Result<(), WireError> {
        match wt {
            WireType::Varint => out.push(self.varint()? as i64),
            WireType::Len => {
                let raw = self.len_delimited()?;
                let mut inner = Reader::nested(raw, self.depth);
                while inner.pos < raw.len() {
                    out.push(inner.varint()? as i64);
                }
            }
            other => return Err(WireError::WrongWireType { field, got: other, want: WireType::Varint }),
        }
        Ok(())
    }
}

/// A message that encodes to and decodes from protobuf fields.
pub trait Message: Sized {
    fn encode(&self, w: &mut Writer) -> Result<(), WireError>;
    fn decode(r: &mut Reader<'_>) -> Result<Self, WireError>;

    fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let mut w = Writer::new();
        self.encode(&mut w)?;
        Ok(w.finish())
    }

    fn from_bytes(b: &[u8]) -> Result<Self, WireError> {
        Self::decode(&mut Reader::new(b))
    }
}

/// Helper for the `required` check on decode: turns an absent field into `MissingField`.
pub fn required<T>(v: Option<T>, name: &'static str) -> Result<T, WireError> {
    v.ok_or(WireError::MissingField(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe {
        v: i64,
    }

    impl Message for Probe {
        fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
            w.int64(1, self.v);
            Ok(())
        }
        fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
            let mut v = 0;
            while let Some((f, wt)) = r.next_tag()? {
                match f {
                    1 => v = r.int64(f, wt)?,
                    _ => r.skip(wt)?,
                }
            }
            Ok(Self { v })
        }
    }

    fn enc_varint(v: u64) -> Vec<u8> {
        let mut w = Writer::new();
        w.varint(v);
        w.finish()
    }

    #[test]
    fn wire_varint_roundtrip() {
        for v in [0u64, 1, 127, 128, 300, u64::MAX] {
            let b = enc_varint(v);
            assert_eq!(Reader::new(&b).varint(), Ok(v));
        }
    }

    #[test]
    fn wire_negative_int64_is_10_bytes() {
        let mut w = Writer::new();
        w.int64(1, -1);
        assert_eq!(w.finish(), [0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]);
    }

    #[test]
    fn wire_negative_int32_is_sign_extended() {
        let mut a = Writer::new();
        a.int32(1, -1);
        let mut b = Writer::new();
        b.int64(1, -1);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn wire_fixed32_float_le() {
        let mut w = Writer::new();
        w.float32(1, 1.5);
        assert_eq!(w.finish(), [0x0D, 0x00, 0x00, 0xC0, 0x3F]);
    }

    #[test]
    fn wire_fixed64_double_le() {
        let mut w = Writer::new();
        w.float64(1, -1.0);
        assert_eq!(w.finish(), [0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xBF]);
    }

    #[test]
    fn wire_string_utf8() {
        let s = "héllo 🙂";
        let mut w = Writer::new();
        w.string(2, s);
        let b = w.finish();
        let mut r = Reader::new(&b);
        let (f, wt) = r.next_tag().unwrap().unwrap();
        assert_eq!(r.string(f, wt).unwrap(), s);
        let bad = [0x12, 0x01, 0xFF];
        let mut r = Reader::new(&bad);
        let (f, wt) = r.next_tag().unwrap().unwrap();
        assert_eq!(r.string(f, wt), Err(WireError::Utf8(2)));
    }

    #[test]
    fn wire_unknown_field_skipped() {
        let mut base = Writer::new();
        base.int64(1, 7);
        let known = base.finish();
        // field 999 as varint, len, fixed32 and fixed64, placed before and after the known field.
        let mut extra = Writer::new();
        extra.int64(999, 5);
        extra.bytes(999, b"abc");
        extra.float32(999, 1.0);
        extra.float64(999, 2.0);
        let mut mixed = extra.finish();
        mixed.extend_from_slice(&known);
        assert_eq!(Probe::from_bytes(&mixed).unwrap().v, 7);
        assert_eq!(Probe::from_bytes(&known).unwrap().v, 7);
    }

    #[test]
    fn wire_field_zero_rejected() {
        assert_eq!(Reader::new(&[0x00, 0x01]).next_tag(), Err(WireError::FieldZero));
    }

    #[test]
    fn wire_truncated_len() {
        let b = [0x0A, 0x10, 0x01];
        let mut r = Reader::new(&b);
        let (_, wt) = r.next_tag().unwrap().unwrap();
        assert_eq!(r.skip(wt), Err(WireError::Truncated));
    }

    #[test]
    fn wire_varint_too_long() {
        let b = [0x80u8; 11];
        assert_eq!(Reader::new(&b).varint(), Err(WireError::VarintTooLong));
    }

    #[test]
    fn wire_wrong_type_known_field() {
        let b = [0x0A, 0x00];
        let mut r = Reader::new(&b);
        let (f, wt) = r.next_tag().unwrap().unwrap();
        assert!(matches!(r.int64(f, wt), Err(WireError::WrongWireType { field: 1, .. })));
    }

    #[test]
    fn wire_packed_and_unpacked_repeated() {
        let mut unpacked = Writer::new();
        unpacked.int64(17, 2);
        unpacked.int64(17, 300);
        let mut packed_inner = Writer::new();
        packed_inner.buf.extend_from_slice(&enc_varint(2));
        packed_inner.buf.extend_from_slice(&enc_varint(300));
        let mut packed = Writer::new();
        packed.bytes(17, &packed_inner.buf);
        for bytes in [unpacked.finish(), packed.finish()] {
            let mut out = Vec::new();
            let mut r = Reader::new(&bytes);
            while let Some((f, wt)) = r.next_tag().unwrap() {
                r.repeated_int64(f, wt, &mut out).unwrap();
            }
            assert_eq!(out, vec![2, 300]);
        }
    }

    #[test]
    fn wire_nesting_depth_limited() {
        // A message nesting itself deeper than the limit must fail rather than overflow the stack.
        struct Deep;
        impl Message for Deep {
            fn encode(&self, _: &mut Writer) -> Result<(), WireError> {
                Ok(())
            }
            fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
                while let Some((f, wt)) = r.next_tag()? {
                    if f == 1 {
                        r.message::<Deep>(f, wt)?;
                    } else {
                        r.skip(wt)?;
                    }
                }
                Ok(Deep)
            }
        }
        let mut payload = Vec::new();
        for _ in 0..100 {
            let mut w = Writer::new();
            w.bytes(1, &payload);
            payload = w.finish();
        }
        assert_eq!(Deep::from_bytes(&payload).err(), Some(WireError::TooDeep));
    }

    #[test]
    fn wire_random_inputs_never_panic() {
        // Deterministic xorshift; 10,000 inputs of varied length.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..10_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let len = (seed % 64) as usize;
            let mut input = Vec::with_capacity(len);
            for _ in 0..len {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                input.push(seed as u8);
            }
            let _ = Probe::from_bytes(&input);
        }
    }
}
