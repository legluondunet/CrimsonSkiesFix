// Read-only interpreter of the two EAX-only unSafeDisc modifier
// functions observed in the owner's Windows 98 memory snapshot.
// No code bytes from the proprietary DLL are embedded here.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This is NOT a SafeDisc unpacker: only a narrow, observed extra
// DWORD layer is modeled. Callers must validate snapshot SHA-256,
// PE geometry, and relevant relocation ranges BEFORE decoding.

pub const FIRST_RVA: usize = 0x65BE;
pub const SECOND_RVA: usize = 0x6621;
pub const END_RVA: usize = 0x6684;
pub const SNAPSHOT_SIZE: usize = 0x29000;
pub const VERIFIED_SNAPSHOT_SHA256: &str =
    "5931af1ba40b0f28b0bb3e9fabc663127bbd9a8c1de8ea1e9c87b56154c84519";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Nop, Inc, Dec, Neg,
    Add(u32), Sub(u32), Xor(u32),
    Rol(u32), Ror(u32),
}

#[derive(Debug, Clone)]
pub struct Modifier {
    ops: Vec<Op>,
}

impl Modifier {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != SECOND_RVA-FIRST_RVA {
            return Err("expected an exactly 99-byte EAX modifier".into());
        }
        let mut ops = Vec::new();
        let mut p = 0usize;
        while p < bytes.len() {
            if bytes[p] == 0xC3 {
                if p+1 != bytes.len() {
                    return Err("RET occurs before the end of the modifier".into());
                }
                return Ok(Self { ops });
            }
            let op = match bytes[p] {
                0x90 => { p+=1; Op::Nop }
                0x40 => { p+=1; Op::Inc }
                0x48 => { p+=1; Op::Dec }
                0xF7 if bytes.get(p+1)==Some(&0xD8) => {
                    p+=2; Op::Neg
                }
                0xC1 => {
                    let kind = *bytes.get(p+1).ok_or("truncated rotate")?;
                    let count = u32::from(*bytes.get(p+2).ok_or("truncated rotate count")?);
                    p+=3;
                    match kind {
                        0xC0 => Op::Rol(count),
                        0xC8 => Op::Ror(count),
                        _ => return Err("non-EAX rotate instruction".into()),
                    }
                }
                0x05|0x2D|0x35 => {
                    let raw = bytes.get(p+1..p+5).ok_or("truncated immediate")?;
                    let imm = u32::from_le_bytes(raw.try_into()
                        .map_err(|_|"invalid immediate width")?);
                    let opcode = bytes[p];
                    p+=5;
                    match opcode {
                        0x05 => Op::Add(imm),
                        0x2D => Op::Sub(imm),
                        _ => Op::Xor(imm),
                    }
                }
                opcode => return Err(format!(
                    "unsupported modifier instruction 0x{opcode:02X} at +0x{p:X}"
                )),
            };
            ops.push(op);
        }
        Err("modifier has no terminal RET".into())
    }

    pub fn operations(&self) -> usize { self.ops.len() }

    pub fn forward(&self, value: u32) -> u32 {
        self.ops.iter().fold(value, |v, op| match *op {
            Op::Nop => v,
            Op::Inc => v.wrapping_add(1),
            Op::Dec => v.wrapping_sub(1),
            Op::Neg => v.wrapping_neg(),
            Op::Add(n) => v.wrapping_add(n),
            Op::Sub(n) => v.wrapping_sub(n),
            Op::Xor(n) => v^n,
            Op::Rol(n) => v.rotate_left(n),
            Op::Ror(n) => v.rotate_right(n),
        })
    }

    pub fn inverse(&self, value: u32) -> u32 {
        self.ops.iter().rev().fold(value, |v, op| match *op {
            Op::Nop => v,
            Op::Inc => v.wrapping_sub(1),
            Op::Dec => v.wrapping_add(1),
            Op::Neg => v.wrapping_neg(),
            Op::Add(n) => v.wrapping_sub(n),
            Op::Sub(n) => v.wrapping_add(n),
            Op::Xor(n) => v^n,
            Op::Rol(n) => v.rotate_right(n),
            Op::Ror(n) => v.rotate_left(n),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Pair {
    pub first: Modifier,
    pub second: Modifier,
}

impl Pair {
    /// Input must be a complete, RVA-addressed *mapped* PE image.
    /// This decoder does not by itself establish that the image is
    /// the verified capture; use a strict SHA/geometry gate.
    pub fn from_mapped_image(image: &[u8]) -> Result<Self,String> {
        let first = image.get(FIRST_RVA..SECOND_RVA)
            .ok_or("first modifier not mapped")?;
        let second = image.get(SECOND_RVA..END_RVA)
            .ok_or("second modifier not mapped")?;
        Ok(Self {
            first: Modifier::decode(first)?,
            second: Modifier::decode(second)?,
        })
    }

    pub fn index_mask(i:u32) -> u32 {
        (i<<24)|(i<<16)|(i<<8)|i
    }

    /// Historical 0x401F62 loop; conditional on EDX and ECX
    /// preservation by the two observed EAX-only functions.
    pub fn extra_word(&self, value:u32, index:u32) -> u32 {
        let m=Self::index_mask(index);
        self.second.forward(self.first.forward(value^m)^m)
    }

    pub fn inverse_extra_word(&self, value:u32, index:u32) -> u32 {
        let m=Self::index_mask(index);
        self.first.inverse(self.second.inverse(value)^m)^m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy_pair() -> Pair {
        let mut image=vec![0u8;END_RVA];
        image[FIRST_RVA..SECOND_RVA-1].fill(0x40);
        image[SECOND_RVA-1]=0xC3;
        image[SECOND_RVA..END_RVA-1].fill(0x48);
        image[END_RVA-1]=0xC3;
        Pair::from_mapped_image(&image).unwrap()
    }

    #[test]
    fn strict_decoder_rejects_illegal_or_truncated() {
        for bad in [0x00u8,0x8B,0xE8,0xFF] {
            let mut data=vec![0x90;99];
            data[7]=bad;
            data[98]=0xC3;
            assert!(Modifier::decode(&data).is_err());
        }
        let mut data=vec![0x90;99];
        data[1]=0xC3; data[98]=0xC3;
        assert!(Modifier::decode(&data).is_err());
        assert!(Modifier::decode(&[0xC3]).is_err());
        data[1]=0x90;
        data[98]=0xC1; // truncated rotation
        assert!(Modifier::decode(&data).is_err());
    }

    #[test]
    fn synthetic_pair_is_reversible() {
        let pair=toy_pair();
        assert_eq!(pair.first.operations(),98);
        assert_eq!(pair.second.operations(),98);
        for src in [0,1,0x12345678,u32::MAX] {
            for i in [0,1,255,256,257,0xABCD1234,u32::MAX] {
                let encoded=pair.extra_word(src,i);
                assert_eq!(pair.inverse_extra_word(encoded,i),src);
            }
        }
    }

    #[test]
    fn x86_full_width_mask_not_byte_truncated() {
        assert_eq!(Pair::index_mask(255),0xFFFFFFFF);
        assert_eq!(Pair::index_mask(256),0x01010100);
        assert_eq!(Pair::index_mask(257),0x01010101);
        assert_eq!(Pair::index_mask(1),Pair::index_mask(257));
    }

    #[test]
    fn rotation_count_uses_low_five_bits() {
        let mut bytes=vec![0x90;99];
        bytes[0]=0xC1;bytes[1]=0xC0;bytes[2]=0xA3;
        bytes[98]=0xC3;
        let decoded=Modifier::decode(&bytes).unwrap();
        assert_eq!(decoded.forward(1),1u32.rotate_left(3));
        assert_eq!(decoded.inverse(decoded.forward(0x12345678)),0x12345678);
    }

    #[test]
    fn fails_closed_on_missing_mapped_rva_region() {
        assert!(Pair::from_mapped_image(&[0u8;1024]).is_err());
    }
}
