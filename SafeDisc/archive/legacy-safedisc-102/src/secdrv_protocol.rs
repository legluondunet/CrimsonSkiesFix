// SafeDisc 1.x secdrv.sys main IOCTL protocol: offline, side-effect-free
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Behavior independently ported/adapted from RibShark/SafeDiscShim
// (GPL-3.0, copyright (C) 2024 RibShark):
// https://github.com/RibShark/SafeDiscShim/blob/main/src/secdrv_ioctl.cpp
// https://github.com/RibShark/SafeDiscShim/blob/main/src/secdrv_ioctl.h
//
// Research only: no Windows hooks, kernel drivers, disc checks or game files.
// The caller must provide actual request bytes; a synthetic test packet
// does NOT demonstrate that the game's SafeDisc decryption is functional.

pub const MAIN_IOCTL: u32 = 0xEF00_2407;
pub const INPUT_BYTES: usize = 0x514;
pub const OUTPUT_BYTES: usize = 0x610;
const VERSION: [u32; 3] = [4, 3, 86];
const REQUEST_COMMAND_OFFSET: usize = 0x0C;
const EXTRA_DATA_OFFSET: usize = 0x414;
const OUTPUT_EXTRA_SIZE_OFFSET: usize = 0x40C;
const OUTPUT_EXTRA_OFFSET: usize = 0x410;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoctlError {
    WrongIoctl,
    InvalidInputSize,
    InvalidOutputSize,
    UnsupportedCommand(u32),
    InvalidArguments(u32),
}

impl std::fmt::Display for IoctlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn u32_at(buffer: &[u8], offset: usize) -> u32 {
    let mut four = [0u8; 4];
    four.copy_from_slice(&buffer[offset..offset + 4]);
    u32::from_le_bytes(four)
}

fn set_u32(buffer: &mut [u8], offset: usize, number: u32) {
    buffer[offset..offset + 4].copy_from_slice(&number.to_le_bytes());
}

fn verification_data(output: &mut [u8], tick_count: u32) {
    let mut value = 0xF367_AC7Fu32;
    let mut checksum = tick_count;
    // Includes only the fields used by SafeDiscShim for SafeDisc 1.
    // The caller-supplied tick count is deliberately deterministic here;
    // a future real adapter must supply a verified live value.
    for index in (1usize..=3).rev() {
        value = 0x3619_62E9u32.wrapping_sub(0x0D5A_CB1Bu32.wrapping_mul(value));
        set_u32(output, 12 + 4 * index, value);
        checksum ^= value;
    }
    set_u32(output, 12, checksum);
}

/// Reproduce the documented subset of the SafeDisc v1 main driver command.
///
/// Output is returned as a new zero-initialized packet. This must not be
/// wired to a live game's DeviceIoControl without separately establishing
/// its calling contract, state and required driver behaviors.
pub fn emulate_main(
    control_code: u32,
    input: &[u8],
    requested_output_bytes: usize,
    tick_count: u32,
) -> Result<Vec<u8>, IoctlError> {
    if control_code != MAIN_IOCTL {
        return Err(IoctlError::WrongIoctl);
    }
    if input.len() != INPUT_BYTES {
        return Err(IoctlError::InvalidInputSize);
    }
    if requested_output_bytes != OUTPUT_BYTES && requested_output_bytes != 0xC18 {
        return Err(IoctlError::InvalidOutputSize);
    }
    let command = u32_at(input, REQUEST_COMMAND_OFFSET);
    let mut result = vec![0u8; requested_output_bytes];
    for (index, version) in VERSION.iter().enumerate() {
        set_u32(&mut result, index * 4, *version);
    }

    // Note: commands >= 0x3F have additional constraints in SafeDiscShim.
    // Implementing only the commands validated against the historical v1
    // protocol avoids misleading "success" for unsupported requests.
    let value = match command {
        0x3C => 0x400,       // GetDebugRegisterInfo
        0x3D => 0x2C8,       // GetIdtInfo
        0x3E => 0x5278_D11B, // SetupVerification
        0x3F..=0x43 => {
            return Err(IoctlError::UnsupportedCommand(command));
        }
        _ => {
            return Err(IoctlError::UnsupportedCommand(command));
        }
    };
    // This field occupies the documented extra data area at +0x410.
    // No assumptions are made about the opaque game-specific input bytes
    // beyond the validated request structure.
    let _ = u32_at(input, EXTRA_DATA_OFFSET);
    set_u32(&mut result, OUTPUT_EXTRA_SIZE_OFFSET, 4);
    set_u32(&mut result, OUTPUT_EXTRA_OFFSET, value);
    verification_data(&mut result, tick_count);
    Ok(result)
}

/// Constructs *synthetic* input solely for repeatable unit tests.
/// Never mistake this packet for an observed live dplayerx.dll request.
#[cfg(test)]
fn fixture(command: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; INPUT_BYTES];
    set_u32(&mut bytes, 0, 1);
    set_u32(&mut bytes, 4, 50);
    set_u32(&mut bytes, 8, 20);
    set_u32(&mut bytes, REQUEST_COMMAND_OFFSET, command);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_three_supported_commands_have_expected_response_layout() {
        for (command, expected) in [(0x3C, 0x400), (0x3D, 0x2C8), (0x3E, 0x5278_D11B)] {
            let result = emulate_main(MAIN_IOCTL, &fixture(command), OUTPUT_BYTES, 0x1234_5678)
                .expect("supported command");
            assert_eq!(result.len(), OUTPUT_BYTES);
            assert_eq!([u32_at(&result, 0), u32_at(&result, 4), u32_at(&result, 8)], VERSION);
            assert_eq!(u32_at(&result, OUTPUT_EXTRA_SIZE_OFFSET), 4);
            assert_eq!(u32_at(&result, OUTPUT_EXTRA_OFFSET), expected);
            assert_ne!(u32_at(&result, 12), 0);
        }
    }

    #[test]
    fn rejects_wrong_ioctl_or_buffer_sizes() {
        let packet = fixture(0x3C);
        assert_eq!(emulate_main(0, &packet, OUTPUT_BYTES, 1), Err(IoctlError::WrongIoctl));
        assert_eq!(emulate_main(MAIN_IOCTL, &packet[..5], OUTPUT_BYTES, 1),
                   Err(IoctlError::InvalidInputSize));
        assert_eq!(emulate_main(MAIN_IOCTL, &packet, 5, 1),
                   Err(IoctlError::InvalidOutputSize));
    }

    #[test]
    fn unknown_and_advanced_commands_fail_closed() {
        assert_eq!(emulate_main(MAIN_IOCTL, &fixture(0x42), OUTPUT_BYTES, 1),
                   Err(IoctlError::UnsupportedCommand(0x42)));
        assert_eq!(emulate_main(MAIN_IOCTL, &fixture(0x99), OUTPUT_BYTES, 1),
                   Err(IoctlError::UnsupportedCommand(0x99)));
    }

    #[test]
    fn verification_data_is_deterministic_and_uses_wrapping_arithmetic() {
        let a = emulate_main(MAIN_IOCTL, &fixture(0x3C), OUTPUT_BYTES, 0xFFFF_FFFF).unwrap();
        let b = emulate_main(MAIN_IOCTL, &fixture(0x3C), OUTPUT_BYTES, 0xFFFF_FFFF).unwrap();
        let c = emulate_main(MAIN_IOCTL, &fixture(0x3C), OUTPUT_BYTES, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(u32_at(&a, 12) ^ u32_at(&c, 12), 0xFFFF_FFFF);
    }
}
