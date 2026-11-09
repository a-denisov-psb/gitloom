//! M6-16: release builds of the Windows binary use the GUI subsystem (no
//! console window); debug builds keep the console subsystem.

#![cfg(windows)]
#![allow(clippy::unwrap_used)]

/// `IMAGE_SUBSYSTEM_WINDOWS_GUI`.
const GUI: u16 = 2;
/// `IMAGE_SUBSYSTEM_WINDOWS_CUI`.
const CONSOLE: u16 = 3;

/// Subsystem field of the PE optional header of `exe`.
fn subsystem(exe: &[u8]) -> u16 {
    let at = |offset: usize| u32::from_le_bytes(exe[offset..offset + 4].try_into().unwrap());
    let pe = at(0x3c) as usize;
    assert_eq!(&exe[pe..pe + 4], b"PE\0\0", "not a PE image");
    // Signature (4) + COFF file header (20), then the optional header;
    // `Subsystem` sits at offset 68 in both PE32 and PE32+.
    let field = pe + 4 + 20 + 68;
    u16::from_le_bytes([exe[field], exe[field + 1]])
}

#[test]
fn binary_uses_the_expected_subsystem() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_gitloom")).unwrap();
    let expected = if cfg!(debug_assertions) { CONSOLE } else { GUI };
    assert_eq!(subsystem(&exe), expected);
}
