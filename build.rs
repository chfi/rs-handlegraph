use std::io::{Result, Write};

const fn comp_base(base: u8) -> u8 {
    match base {
        b'A' => b'T',
        b'G' => b'C',
        b'C' => b'G',
        b'T' => b'A',
        b'a' => b't',
        b'g' => b'c',
        b'c' => b'g',
        b't' => b'a',
        _ => b'N',
    }
}

fn main() -> Result<()> {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("comp_table.rs");
    let mut f = std::fs::File::create(&dest_path).unwrap();

    write!(f, "const DNA_COMP_TABLE: [u8; 256] = [\n")?;
    for b in 0..=255u8 {
        write!(f, "  {},\n", (comp_base(b)))?;
    }
    write!(f, "];\n")?;

    Ok(())
}
