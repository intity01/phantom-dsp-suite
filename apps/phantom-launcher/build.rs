//! Pure-Rust Windows `.ico` COFF resource object generator for `phantom-launcher.exe`.
//! Produces an `.rsrc` COFF object compatible with both MSVC `link.exe` and `cargo-zigbuild`/`lld`.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let ico_path = PathBuf::from("assets/icons/icon.ico");
    let ico_bytes = match fs::read(&ico_path) {
        Ok(b) => b,
        Err(e) => {
            println!("cargo:warning=Skipping Windows icon embedding ({e})");
            return;
        }
    };

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".into());
    let (machine, reloc_type) = match arch.as_str() {
        "x86_64" => (0x8664u16, 3u16),  // IMAGE_REL_AMD64_ADDR32NB
        "x86" => (0x014Cu16, 7u16),     // IMAGE_REL_I386_DIR32NB
        "aarch64" => (0xAA64u16, 2u16), // IMAGE_REL_ARM64_ADDR32NB
        _ => return,
    };

    let Ok(obj_bytes) = build_icon_coff(&ico_bytes, machine, reloc_type) else {
        println!("cargo:warning=Failed to parse icon.ico for Windows resource embedding");
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let obj_path = out_dir.join("phantom_icon.obj");
    fs::write(&obj_path, obj_bytes).expect("failed writing phantom_icon.obj");
    println!(
        "cargo:rustc-link-arg-bin=phantom-launcher={}",
        obj_path.display()
    );
}

fn read_u16(b: &[u8], off: usize) -> Result<u16, ()> {
    let s = b.get(off..off + 2).ok_or(())?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(b: &[u8], off: usize) -> Result<u32, ()> {
    let s = b.get(off..off + 4).ok_or(())?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn push_dir_header(buf: &mut Vec<u8>, id_entries: u16) {
    buf.extend_from_slice(&[0u8; 14]);
    buf.extend_from_slice(&id_entries.to_le_bytes());
}

fn push_dir_entry(buf: &mut Vec<u8>, id: u32, offset_or_sub: u32) {
    buf.extend_from_slice(&id.to_le_bytes());
    buf.extend_from_slice(&offset_or_sub.to_le_bytes());
}

fn pad4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn build_icon_coff(ico: &[u8], machine: u16, reloc_type: u16) -> Result<Vec<u8>, ()> {
    if read_u16(ico, 0)? != 0 || read_u16(ico, 2)? != 1 {
        return Err(());
    }
    let count = read_u16(ico, 4)? as usize;
    if count == 0 || ico.len() < 6 + count * 16 {
        return Err(());
    }

    let mut grp_dir = Vec::with_capacity(6 + count * 14);
    grp_dir.extend_from_slice(&0u16.to_le_bytes());
    grp_dir.extend_from_slice(&1u16.to_le_bytes());
    grp_dir.extend_from_slice(&(count as u16).to_le_bytes());

    let mut images: Vec<&[u8]> = Vec::with_capacity(count);
    for idx in 0..count {
        let e_off = 6 + idx * 16;
        let size = read_u32(ico, e_off + 8)? as usize;
        let img_off = read_u32(ico, e_off + 12)? as usize;
        let slice = ico.get(img_off..img_off + size).ok_or(())?;
        images.push(slice);
        grp_dir.extend_from_slice(&ico[e_off..e_off + 12]);
        grp_dir.extend_from_slice(&((idx + 1) as u16).to_le_bytes());
    }

    // Layout offsets inside `.rsrc`
    let root_off = 0usize;
    let root_len = 16 + 2 * 8;
    let type_icon_off = root_off + root_len;
    let type_icon_len = 16 + count * 8;
    let type_grp_off = type_icon_off + type_icon_len;
    let type_grp_len = 16 + 8;
    let langs_off = type_grp_off + type_grp_len;
    let lang_len = 24usize;
    let data_entries_off = langs_off + (count + 1) * lang_len;
    let total_data_entries = count + 1;

    let mut rsrc = Vec::new();
    // Level 1: root directory (3 = RT_ICON, 14 = RT_GROUP_ICON)
    push_dir_header(&mut rsrc, 2);
    push_dir_entry(&mut rsrc, 3, 0x8000_0000 | (type_icon_off as u32));
    push_dir_entry(&mut rsrc, 14, 0x8000_0000 | (type_grp_off as u32));

    // Level 2: RT_ICON IDs (1..=count)
    push_dir_header(&mut rsrc, count as u16);
    for idx in 0..count {
        let lang_off = langs_off + idx * lang_len;
        push_dir_entry(&mut rsrc, (idx + 1) as u32, 0x8000_0000 | (lang_off as u32));
    }

    // Level 2: RT_GROUP_ICON ID (1)
    push_dir_header(&mut rsrc, 1);
    let grp_lang_off = langs_off + count * lang_len;
    push_dir_entry(&mut rsrc, 1, 0x8000_0000 | (grp_lang_off as u32));

    // Level 3: Language directories (0x0409 = en-US)
    for idx in 0..total_data_entries {
        push_dir_header(&mut rsrc, 1);
        let entry_off = data_entries_off + idx * 16;
        push_dir_entry(&mut rsrc, 0x0409, entry_off as u32);
    }

    // Placeholder IMAGE_RESOURCE_DATA_ENTRY records (filled below)
    rsrc.resize(data_entries_off + total_data_entries * 16, 0);

    let mut relocs = Vec::with_capacity(total_data_entries);
    for (idx, img) in images.iter().enumerate() {
        pad4(&mut rsrc);
        let blob_off = rsrc.len() as u32;
        rsrc.extend_from_slice(img);
        let rec_off = data_entries_off + idx * 16;
        rsrc[rec_off..rec_off + 4].copy_from_slice(&blob_off.to_le_bytes());
        rsrc[rec_off + 4..rec_off + 8].copy_from_slice(&(img.len() as u32).to_le_bytes());
        relocs.push(rec_off as u32);
    }

    pad4(&mut rsrc);
    let grp_off = rsrc.len() as u32;
    rsrc.extend_from_slice(&grp_dir);
    let grp_rec_off = data_entries_off + count * 16;
    rsrc[grp_rec_off..grp_rec_off + 4].copy_from_slice(&grp_off.to_le_bytes());
    rsrc[grp_rec_off + 4..grp_rec_off + 8].copy_from_slice(&(grp_dir.len() as u32).to_le_bytes());
    relocs.push(grp_rec_off as u32);
    pad4(&mut rsrc);

    // Assemble COFF object
    let raw_off = 20u32 + 40u32;
    let reloc_off = raw_off + rsrc.len() as u32;
    let symtab_off = reloc_off + (relocs.len() as u32) * 10;

    let mut obj = Vec::with_capacity(symtab_off as usize + 18 + 4);
    // COFF File Header (20 bytes)
    obj.extend_from_slice(&machine.to_le_bytes());
    obj.extend_from_slice(&1u16.to_le_bytes());
    obj.extend_from_slice(&0u32.to_le_bytes());
    obj.extend_from_slice(&symtab_off.to_le_bytes());
    obj.extend_from_slice(&1u32.to_le_bytes());
    obj.extend_from_slice(&0u16.to_le_bytes());
    obj.extend_from_slice(&0u16.to_le_bytes());

    // Section Header ".rsrc" (40 bytes)
    obj.extend_from_slice(b".rsrc\0\0\0");
    obj.extend_from_slice(&0u32.to_le_bytes());
    obj.extend_from_slice(&0u32.to_le_bytes());
    obj.extend_from_slice(&(rsrc.len() as u32).to_le_bytes());
    obj.extend_from_slice(&raw_off.to_le_bytes());
    obj.extend_from_slice(&reloc_off.to_le_bytes());
    obj.extend_from_slice(&0u32.to_le_bytes());
    obj.extend_from_slice(&(relocs.len() as u16).to_le_bytes());
    obj.extend_from_slice(&0u16.to_le_bytes());
    obj.extend_from_slice(&0x4000_0040u32.to_le_bytes());

    obj.extend_from_slice(&rsrc);
    for va in relocs {
        obj.extend_from_slice(&va.to_le_bytes());
        obj.extend_from_slice(&0u32.to_le_bytes());
        obj.extend_from_slice(&reloc_type.to_le_bytes());
    }

    // Symbol Table: Symbol 0 = ".rsrc" section static symbol (18 bytes) + 4-byte empty string table
    obj.extend_from_slice(b".rsrc\0\0\0");
    obj.extend_from_slice(&0u32.to_le_bytes());
    obj.extend_from_slice(&1i16.to_le_bytes());
    obj.extend_from_slice(&0u16.to_le_bytes());
    obj.push(3); // IMAGE_SYM_CLASS_STATIC
    obj.push(0);
    obj.extend_from_slice(&4u32.to_le_bytes());

    Ok(obj)
}
