fn main() {
    embuild::espidf::sysenv::output();

    // 1. Build the auxiliary RP2040-Zero USB-MIDI coprocessor firmware
    println!("cargo:rerun-if-changed=../rp2040-usb-midi/src");
    println!("cargo:rerun-if-changed=../rp2040-usb-midi/Cargo.toml");
    println!("cargo:rerun-if-changed=../rp2040-usb-midi/memory.x");

    let rp2040_dir = std::path::Path::new("../rp2040-usb-midi")
        .canonicalize()
        .expect("rp2040-usb-midi directory must exist");

    let status = std::process::Command::new("cargo")
        .arg("+stable")
        .arg("build")
        .arg("--release")
        .arg("--offline")
        .current_dir(&rp2040_dir)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("TARGET")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("CARGO")
        .env_remove("CLIPPY_ARGS")
        .status()
        .expect("Failed to execute cargo build for rp2040-usb-midi");

    assert!(status.success(), "cargo build for rp2040-usb-midi failed");

    // 2. Extract raw binary from the 32-bit ELF output (PT_LOAD segments @ 0x1000_4000)
    let elf_path = rp2040_dir.join("target/thumbv6m-none-eabi/release/rp2040-usb-midi");
    let elf = std::fs::read(&elf_path).expect("Read rp2040-usb-midi ELF");
    assert_eq!(&elf[0..4], b"\x7fELF", "Not an ELF file");
    assert_eq!(elf[4], 1, "Not a 32-bit ELF");

    let phoff = u32::from_le_bytes(elf[28..32].try_into().expect("ELF phoff")) as usize;
    let phentsize = u16::from_le_bytes(elf[42..44].try_into().expect("ELF phentsize")) as usize;
    let phnum = u16::from_le_bytes(elf[44..46].try_into().expect("ELF phnum")) as usize;

    let base_addr = 0x1000_4000u32;
    let mut raw_bin = Vec::new();

    for i in 0..phnum {
        let offset = phoff + i * phentsize;
        let ph = &elf[offset..offset + phentsize];

        let p_type = u32::from_le_bytes(ph[0..4].try_into().expect("p_type"));
        let p_offset = u32::from_le_bytes(ph[4..8].try_into().expect("p_offset")) as usize;
        let p_paddr = u32::from_le_bytes(ph[12..16].try_into().expect("p_paddr"));
        let p_filesz = u32::from_le_bytes(ph[16..20].try_into().expect("p_filesz")) as usize;

        // PT_LOAD is 1. Extract Flash segments located in 0x1000_4000..0x2000_0000
        if p_type == 1 && p_filesz > 0 && p_paddr >= base_addr && p_paddr < 0x2000_0000 {
            let start = (p_paddr - base_addr) as usize;
            let end = start + p_filesz;
            if raw_bin.len() < end {
                raw_bin.resize(end, 0x00);
            }
            raw_bin[start..end].copy_from_slice(&elf[p_offset..p_offset + p_filesz]);
        }
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by Cargo");
    let out_path = std::path::Path::new(&out_dir).join("rp2040-usb-midi.bin");
    std::fs::write(&out_path, &raw_bin).expect("Write rp2040-usb-midi.bin to OUT_DIR");

    // Also update target release folder for reference
    let _ = std::fs::write(
        rp2040_dir.join("target/thumbv6m-none-eabi/release/rp2040-usb-midi.bin"),
        &raw_bin,
    );

    // 3. Compile Slint UI
    // Slint is always compiled.
    // Without Slint's `std` feature there is no system font database on the
    // target, so fonts/images are pre-rendered and embedded at build time.
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("ui/index.slint", config)
        .expect("Slint UI compilation failed");
}
