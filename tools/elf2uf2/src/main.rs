//! Pure Rust zero-dependency ELF to UF2 converter for Raspberry Pi RP2040.
//!
//! Converts standard 32-bit Cortex-M0+ ELF binaries into USB Flashing Format (UF2)
//! blocks compatible with the RP2040 Bootrom mass storage bootloader.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const UF2_MAGIC_START0: u32 = 0x0A32_4655;
const UF2_MAGIC_START1: u32 = 0x9E5D_5157;
const UF2_MAGIC_END: u32 = 0x0AB1_6F30;
const UF2_FLAG_FAMILY_ID_PRESENT: u32 = 0x0000_2000;
const RP2040_FAMILY_ID: u32 = 0xE48B_FF56;
const FLASH_PAGE_SIZE: usize = 256;
const UF2_BLOCK_SIZE: usize = 512;
const RP2040_FLASH_BASE: u32 = 0x1000_0000;
const RP2040_FLASH_LIMIT: u32 = 0x2000_0000;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: elf2uf2 <input.elf> [output.uf2]");
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let output_path = if args.len() >= 3 {
        PathBuf::from(&args[2])
    } else {
        input_path.with_extension("uf2")
    };

    println!("Converting {:?} to {:?}...", input_path, output_path);
    convert_elf_to_uf2(input_path, &output_path)?;
    println!("Successfully generated UF2 at {:?}", output_path);
    Ok(())
}

/// Reads an ELF file from disk and writes the resulting UF2 image.
pub fn convert_elf_to_uf2(elf_path: &Path, uf2_path: &Path) -> Result<(), Box<dyn Error>> {
    let elf_bytes = fs::read(elf_path)?;
    let uf2_output = convert_elf_bytes_to_uf2(&elf_bytes)?;
    fs::write(uf2_path, &uf2_output)?;
    println!(
        "UF2 file written: {} blocks ({} bytes)",
        uf2_output.len() / UF2_BLOCK_SIZE,
        uf2_output.len()
    );
    Ok(())
}

/// Converts in-memory ELF file bytes into in-memory UF2 format bytes.
pub fn convert_elf_bytes_to_uf2(elf_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if elf_bytes.len() < 52 {
        return Err("ELF file too short to contain a valid header".into());
    }

    // Validate ELF magic: 0x7F, 'E', 'L', 'F'
    if &elf_bytes[0..4] != b"\x7fELF" {
        return Err("Not a valid ELF file".into());
    }
    // 32-bit format
    if elf_bytes[4] != 1 {
        return Err("Unsupported ELF class (expected 32-bit)".into());
    }
    // Little-endian
    if elf_bytes[5] != 1 {
        return Err("Unsupported ELF endianness (expected little-endian)".into());
    }

    // Program header table offset and counts
    let phoff = u32::from_le_bytes(
        elf_bytes[28..32]
            .try_into()
            .map_err(|_| "Failed to parse phoff")?,
    ) as usize;
    let phentsize = u16::from_le_bytes(
        elf_bytes[42..44]
            .try_into()
            .map_err(|_| "Failed to parse phentsize")?,
    ) as usize;
    let phnum = u16::from_le_bytes(
        elf_bytes[44..46]
            .try_into()
            .map_err(|_| "Failed to parse phnum")?,
    ) as usize;

    if phentsize < 32 {
        return Err("Invalid program header entry size (minimum 32 bytes)".into());
    }

    // Collect all PT_LOAD segments destined for Flash (0x10000000 .. 0x20000000)
    struct Segment {
        paddr: u32,
        offset: usize,
        filesz: usize,
    }

    let mut segments = Vec::new();
    let mut min_paddr = u32::MAX;
    let mut max_paddr = 0u32;

    for i in 0..phnum {
        let entry_offset = phoff
            .checked_add(i * phentsize)
            .ok_or("Program header offset calculation overflow")?;
        if entry_offset + 32 > elf_bytes.len() {
            return Err("Program header table exceeds file size".into());
        }
        let ph = &elf_bytes[entry_offset..entry_offset + 32];

        let p_type = u32::from_le_bytes(ph[0..4].try_into().map_err(|_| "p_type")?);
        let p_offset = u32::from_le_bytes(ph[4..8].try_into().map_err(|_| "p_offset")?) as usize;
        let p_vaddr = u32::from_le_bytes(ph[8..12].try_into().map_err(|_| "p_vaddr")?);
        let p_paddr = u32::from_le_bytes(ph[12..16].try_into().map_err(|_| "p_paddr")?);
        let p_filesz = u32::from_le_bytes(ph[16..20].try_into().map_err(|_| "p_filesz")?) as usize;

        // Use physical load address (LMA); fallback to virtual address (VMA) if paddr is 0
        let load_addr = if p_paddr != 0 { p_paddr } else { p_vaddr };

        // PT_LOAD = 1
        if p_type == 1
            && p_filesz > 0
            && (RP2040_FLASH_BASE..RP2040_FLASH_LIMIT).contains(&load_addr)
        {
            let seg_end = p_offset
                .checked_add(p_filesz)
                .ok_or("Segment payload offset calculation overflow")?;
            if seg_end > elf_bytes.len() {
                return Err("Segment data exceeds file boundary".into());
            }

            if load_addr < min_paddr {
                min_paddr = load_addr;
            }
            let end_addr = load_addr
                .checked_add(p_filesz as u32)
                .ok_or("Segment memory address calculation overflow")?;
            if end_addr > max_paddr {
                max_paddr = end_addr;
            }

            segments.push(Segment {
                paddr: load_addr,
                offset: p_offset,
                filesz: p_filesz,
            });
        }
    }

    if segments.is_empty() {
        return Err("No flash PT_LOAD segments found in ELF".into());
    }

    // Align base down to 256-byte page boundary
    let aligned_base = min_paddr & !(FLASH_PAGE_SIZE as u32 - 1);
    let total_span = (max_paddr - aligned_base) as usize;
    let num_pages = total_span.div_ceil(FLASH_PAGE_SIZE);

    // Build contiguous binary image
    let mut flash_image = vec![0x00u8; num_pages * FLASH_PAGE_SIZE];
    for seg in &segments {
        let dest_start = (seg.paddr - aligned_base) as usize;
        let dest_end = dest_start + seg.filesz;
        flash_image[dest_start..dest_end]
            .copy_from_slice(&elf_bytes[seg.offset..seg.offset + seg.filesz]);
    }

    // Generate UF2 blocks
    let total_blocks = num_pages as u32;
    let mut uf2_output = Vec::with_capacity(num_pages * UF2_BLOCK_SIZE);

    for (block_idx, chunk) in flash_image.chunks(FLASH_PAGE_SIZE).enumerate() {
        let target_addr = aligned_base + (block_idx * FLASH_PAGE_SIZE) as u32;

        let mut block = [0u8; UF2_BLOCK_SIZE];
        block[0..4].copy_from_slice(&UF2_MAGIC_START0.to_le_bytes());
        block[4..8].copy_from_slice(&UF2_MAGIC_START1.to_le_bytes());
        block[8..12].copy_from_slice(&UF2_FLAG_FAMILY_ID_PRESENT.to_le_bytes());
        block[12..16].copy_from_slice(&target_addr.to_le_bytes());
        block[16..20].copy_from_slice(&(FLASH_PAGE_SIZE as u32).to_le_bytes());
        block[20..24].copy_from_slice(&(block_idx as u32).to_le_bytes());
        block[24..28].copy_from_slice(&total_blocks.to_le_bytes());
        block[28..32].copy_from_slice(&RP2040_FAMILY_ID.to_le_bytes());

        // 256 bytes payload
        block[32..32 + FLASH_PAGE_SIZE].copy_from_slice(chunk);

        // Padding (bytes 288..508) remains 0x00

        // End magic
        block[508..512].copy_from_slice(&UF2_MAGIC_END.to_le_bytes());

        uf2_output.extend_from_slice(&block);
    }

    Ok(uf2_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a minimal valid 32-bit little-endian ELF binary with specified program headers.
    fn make_synthetic_elf(
        headers: &[(u32, u32, u32, &[u8])], // (p_vaddr, p_paddr, p_type, payload)
    ) -> Vec<u8> {
        let mut elf = vec![0u8; 52];
        // e_ident
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 1; // 32-bit
        elf[5] = 1; // little endian
        elf[6] = 1; // ELF version
        elf[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        elf[18..20].copy_from_slice(&0x28u16.to_le_bytes()); // EM_ARM

        let phentsize = 32u16;
        let phnum = headers.len() as u16;
        let phoff = 52u32;

        elf[28..32].copy_from_slice(&phoff.to_le_bytes());
        elf[42..44].copy_from_slice(&phentsize.to_le_bytes());
        elf[44..46].copy_from_slice(&phnum.to_le_bytes());

        let mut payload_offset = (phoff as usize) + (phnum as usize) * (phentsize as usize);
        let mut ph_bytes = Vec::new();
        let mut payload_bytes = Vec::new();

        for (vaddr, paddr, ptype, data) in headers {
            let mut ph = [0u8; 32];
            ph[0..4].copy_from_slice(&ptype.to_le_bytes());
            ph[4..8].copy_from_slice(&(payload_offset as u32).to_le_bytes());
            ph[8..12].copy_from_slice(&vaddr.to_le_bytes());
            ph[12..16].copy_from_slice(&paddr.to_le_bytes());
            ph[16..20].copy_from_slice(&(data.len() as u32).to_le_bytes());
            ph[20..24].copy_from_slice(&(data.len() as u32).to_le_bytes());
            ph[24..28].copy_from_slice(&5u32.to_le_bytes()); // PF_R | PF_X
            ph[28..32].copy_from_slice(&4u32.to_le_bytes()); // align

            ph_bytes.extend_from_slice(&ph);
            payload_bytes.extend_from_slice(data);
            payload_offset += data.len();
        }

        elf.extend_from_slice(&ph_bytes);
        elf.extend_from_slice(&payload_bytes);
        elf
    }

    #[test]
    fn test_valid_synthetic_elf_to_uf2() {
        let boot2_data = vec![0x42u8; 256];
        let text_data = vec![0xAAu8; 512];

        let elf = make_synthetic_elf(&[
            (0x1000_0000, 0x1000_0000, 1, &boot2_data), // .boot2 (1 page)
            (0x1000_0100, 0x1000_0100, 1, &text_data),  // .text (2 pages)
        ]);

        let uf2 = convert_elf_bytes_to_uf2(&elf).expect("Conversion should succeed");
        // Total bytes = 256 + 512 = 768 bytes => 3 pages => 3 * 512 = 1536 bytes
        assert_eq!(uf2.len(), 3 * 512);

        for block_idx in 0..3 {
            let block = &uf2[block_idx * 512..(block_idx + 1) * 512];
            let magic0 = u32::from_le_bytes(block[0..4].try_into().unwrap());
            let magic1 = u32::from_le_bytes(block[4..8].try_into().unwrap());
            let flags = u32::from_le_bytes(block[8..12].try_into().unwrap());
            let target_addr = u32::from_le_bytes(block[12..16].try_into().unwrap());
            let payload_size = u32::from_le_bytes(block[16..20].try_into().unwrap());
            let idx = u32::from_le_bytes(block[20..24].try_into().unwrap());
            let num_blocks = u32::from_le_bytes(block[24..28].try_into().unwrap());
            let family_id = u32::from_le_bytes(block[28..32].try_into().unwrap());
            let magic_end = u32::from_le_bytes(block[508..512].try_into().unwrap());

            assert_eq!(magic0, UF2_MAGIC_START0);
            assert_eq!(magic1, UF2_MAGIC_START1);
            assert_eq!(flags, UF2_FLAG_FAMILY_ID_PRESENT);
            assert_eq!(target_addr, 0x1000_0000 + (block_idx as u32 * 256));
            assert_eq!(payload_size, 256);
            assert_eq!(idx, block_idx as u32);
            assert_eq!(num_blocks, 3);
            assert_eq!(family_id, RP2040_FAMILY_ID);
            assert_eq!(magic_end, UF2_MAGIC_END);

            // Verify payload contents
            if block_idx == 0 {
                assert_eq!(&block[32..288], &boot2_data[..]);
            } else if block_idx == 1 {
                assert_eq!(&block[32..288], &text_data[0..256]);
            } else {
                assert_eq!(&block[32..288], &text_data[256..512]);
            }
        }
    }

    #[test]
    fn test_corrupt_elf_magic() {
        let mut elf = make_synthetic_elf(&[(0x1000_0000, 0x1000_0000, 1, &[1, 2, 3])]);
        elf[0] = 0x00;
        let err = convert_elf_bytes_to_uf2(&elf).unwrap_err();
        assert!(err.to_string().contains("Not a valid ELF file"));
    }

    #[test]
    fn test_unsupported_64bit_elf() {
        let mut elf = make_synthetic_elf(&[(0x1000_0000, 0x1000_0000, 1, &[1, 2, 3])]);
        elf[4] = 2; // 64-bit
        let err = convert_elf_bytes_to_uf2(&elf).unwrap_err();
        assert!(err.to_string().contains("expected 32-bit"));
    }

    #[test]
    fn test_unsupported_big_endian() {
        let mut elf = make_synthetic_elf(&[(0x1000_0000, 0x1000_0000, 1, &[1, 2, 3])]);
        elf[5] = 2; // Big-endian
        let err = convert_elf_bytes_to_uf2(&elf).unwrap_err();
        assert!(err.to_string().contains("expected little-endian"));
    }

    #[test]
    fn test_truncated_elf_file() {
        let elf = vec![0x7f, b'E', b'L', b'F'];
        let err = convert_elf_bytes_to_uf2(&elf).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn test_no_flash_segments() {
        // RAM address 0x20000000 only
        let elf = make_synthetic_elf(&[(0x2000_0000, 0x2000_0000, 1, &[0xDE, 0xAD])]);
        let err = convert_elf_bytes_to_uf2(&elf).unwrap_err();
        assert!(err.to_string().contains("No flash PT_LOAD segments found"));
    }
}
