//! Samsung Portable SSD unlock protocol via bulk transfers.
//!
//! Protocol reference (rsenden/t3unlock + redchenjs/pssdunlock):
//!   - VID: 0x04e8 (Samsung Electronics)
//!   - PID T1 locked: 0x61f2, normal: 0x61f1
//!   - PID T3 locked: 0x61f4, normal: 0x61f3
//!   - PID T5 locked: 0x61f6, normal: 0x61f5
//!   - Bulk OUT endpoint:  0x02
//!   - Bulk IN endpoint:   0x81
//!   - Interface: 0
//!
//! Unlock sequence per model:
//!   1. Claim interface 0
//!   2. OUT  → 31-byte unlock header  → EP 0x02
//!   3. OUT  → 512-byte password      → EP 0x02
//!   4. IN   ← 512-byte response       → EP 0x81
//!   5. OUT  → 31-byte relink header   → EP 0x02
//!   6. IN   ← 512-byte response       → EP 0x81

use rusb::{DeviceHandle, GlobalContext};
use super::model::Model;
use std::time::Duration;

/// Bulk OUT endpoint (host → device)
pub const EP_OUT: u8 = 0x02;
/// Bulk IN endpoint (device → host)
pub const EP_IN: u8 = 0x81;
/// USB interface for mass-storage protocol
pub const INTERFACE: u8 = 0;

/// Default USB timeout (ms)
const _TIMEOUT_MS: u64 = 3000; // reserved for future timeout config

/// Build the 31-byte unlock header frame.
///
/// Layout:
///   Bytes 0-3:   "USBC" magic  → 55 53 42 43
///   Byte  4:     bRequestType  → 0x0a
///   Bytes 5-7:   zero padding  → 00 00 00
///   Byte  8:     0x00
///   Byte  9:     0x02  (flags)
///   Bytes 10-11: zero         → 00 00
///   Byte  12:    0x00
///   Byte  13:    0x00
///   Byte  14:    0x10
///   Bytes 15-17: 85 0a 26
///   Byte  18:    0x00
///   Byte  19:    param byte   → 0xd6 (= -42 two's complement)
///   Bytes 20-30: rest of frame
///
/// Model-specific variants:
///   T1: bRequestType=0x0a, param=0xd2 (-46), last bytes 2e 00 e1 00
///   T3: bRequestType=0x0a, param=0xd6 (-42), last bytes c6 00 4f 00 c2 00 b0 00
///   T5: bRequestType=0x0a, param=0xde (-34), last bytes ca 00 57 00 ca 00 b1 00
pub fn build_unlock_frame(model: Model) -> [u8; 31] {
    let mut frame = [0u8; 31];
    // Magic
    frame[0] = 0x55;
    frame[1] = 0x53;
    frame[2] = 0x42;
    frame[3] = 0x43;
    // bRequestType
    frame[4] = 0x0a;
    // zero bytes 5-7 already 0
    frame[9] = 0x02;
    frame[14] = 0x10;
    frame[15] = 0x85;
    frame[16] = 0x0a;
    frame[17] = 0x26;
    match model {
        Model::T1 => {
            frame[19] = 0xd2; // -46
            frame[24] = 0x2e;
            frame[25] = 0x00;
            frame[26] = 0xe1;
            frame[27] = 0x00;
        }
        Model::T3 => {
            frame[19] = 0xd6; // -42
            frame[24] = 0xc6;
            frame[25] = 0x00;
            frame[26] = 0x4f;
            frame[27] = 0x00;
            frame[28] = 0xc2;
            frame[29] = 0x00;
            frame[30] = 0xb0;
        }
        Model::T5 => {
            frame[19] = 0xde; // -34
            frame[24] = 0xca;
            frame[25] = 0x00;
            frame[26] = 0x57;
            frame[27] = 0x00;
            frame[28] = 0xca;
            frame[29] = 0x00;
            frame[30] = 0xb1;
        }
    }
    frame
}

/// Build the 31-byte relink (reconnect) header frame.
///
/// Layout:
///   Bytes 0-3:   "USBC" magic  → 55 53 42 43
///   Byte  4:     bRequestType  → 0x0b
///   Bytes 5-7:   zero padding  → 00 00 00
///   Bytes 8-14:  zero
///   Byte  15:    0x06
///   Byte  16:    0xe8
///   Bytes 17-30: zero
pub fn build_relink_frame() -> [u8; 31] {
    let mut frame = [0u8; 31];
    frame[0] = 0x55;
    frame[1] = 0x53;
    frame[2] = 0x42;
    frame[3] = 0x43;
    frame[4] = 0x0b;
    frame[15] = 0x06;
    frame[16] = 0xe8;
    frame
}

/// Build a 512-byte password frame (zero-padded ASCII).
pub fn build_password_frame(password: &[u8]) -> [u8; 512] {
    let mut frame = [0u8; 512];
    let len = password.len().min(512);
    frame[..len].copy_from_slice(&password[..len]);
    frame
}

/// Check if the 512-byte response indicates success or failure.
/// Byte 9 == 0x02 means failure.
pub fn response_failed(response: &[u8; 512]) -> bool {
    response.get(9).copied() == Some(0x02)
}

/// Send unlock sequence: unlock frame → password frame → relink frame.
/// All transfers use bulk endpoints (OUT 0x02 / IN 0x81).
pub fn unlock(
    handle: &mut DeviceHandle<GlobalContext>,
    model: Model,
    password: &[u8],
    timeout: Duration,
) -> anyhow::Result<()> {
    let unlock_frame = build_unlock_frame(model);
    let password_frame = build_password_frame(password);
    let relink_frame = build_relink_frame();
    let mut response = [0u8; 512];

    // Step 1: send unlock header
    bulk_write(handle, EP_OUT, &unlock_frame, timeout)?;
    tracing::debug!("unlock header sent");

    // Step 2: send password frame
    bulk_write(handle, EP_OUT, &password_frame, timeout)?;
    tracing::debug!("password frame sent");

    // Step 3: read response
    let n = bulk_read(handle, EP_IN, &mut response, timeout)?;
    tracing::debug!("password response {} bytes, byte9={:02x}", n, response[9]);
    if response_failed(&response) {
        anyhow::bail!("password rejected by device");
    }

    // Step 4: send relink header
    bulk_write(handle, EP_OUT, &relink_frame, timeout)?;
    tracing::debug!("relink header sent");

    // Step 5: read relink response
    let n = bulk_read(handle, EP_IN, &mut response, timeout)?;
    tracing::debug!("relink response {} bytes, byte9={:02x}", n, response[9]);
    if response_failed(&response) {
        anyhow::bail!("relink rejected by device");
    }

    Ok(())
}

/// Bulk write to the device.
fn bulk_write(handle: &mut DeviceHandle<GlobalContext>, ep: u8, data: &[u8], timeout: Duration) -> anyhow::Result<usize> {
    handle.write_bulk(ep, data, timeout)
        .map_err(|e| anyhow::anyhow!("bulk write error: {}", e))
}

/// Bulk read from the device.
fn bulk_read(handle: &mut DeviceHandle<GlobalContext>, ep: u8, data: &mut [u8], timeout: Duration) -> anyhow::Result<usize> {
    handle.read_bulk(ep, data, timeout)
        .map_err(|e| anyhow::anyhow!("bulk read error: {}", e))
}
