use sam::{ouch, u32_to_hex};
use std::convert::TryInto;
use std::env;
use std::fs;
use std::io::{self, prelude::*, SeekFrom};

fn read_u32<R: Read>(mut r: R) -> Result<u32, io::Error> {
    let mut buf = Vec::with_capacity(4);
    buf.resize(4, 0);
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf.try_into().unwrap()))
}

fn read_u32_at<F: Read + Seek>(mut f: F, pos: u64) -> Result<u32, io::Error> {
    f.seek(SeekFrom::Start(pos))?;
    read_u32(f)
}

fn read_len_prefixed_str<F: Read + Seek>(mut f: F) -> io::Result<String> {
    let s_len = read_u32(&mut f)?;
    let mut buf = Vec::with_capacity(s_len as usize);
    buf.resize(s_len as usize, 0);
    f.read_exact(&mut buf)?;
    let s = String::from_utf8(buf).unwrap_or_else(ouch);
    let padding_len = ((s_len + 3) & !0b11) - s_len;
    f.seek(SeekFrom::Current(padding_len as i64))?;
    Ok(s)
}

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert!(args.len() >= 2);

    for arg in &args[1..] {
        let mut input = fs::File::open(arg).unwrap_or_else(ouch);

        let mut buf = Vec::with_capacity(0x10);

        // check magic
        buf.resize(16, 0);
        input.read_exact(&mut buf).unwrap_or_else(ouch);
        assert_eq!(&buf, &[
            // magic = dc867b72-87f7-47da-a770-752af3299a3c
            0xdc, 0x86, 0x7b, 0x72, 0x87, 0xf7, 0x47, 0xda,
            0xa7, 0x70, 0x75, 0x2a, 0xf3, 0x29, 0x9a, 0x3c,
        ]);

        // check version
        buf.resize(1, 0);
        input.read_exact(&mut buf).unwrap_or_else(ouch);
        assert_eq!(buf[0], 0x00);

        // dump load address
        let load_address_offset = read_u32_at(&mut input, 0x14).unwrap_or_else(ouch);
        let load_address = read_u32_at(&mut input, load_address_offset as u64).unwrap_or_else(ouch);
        println!("load address = {}", u32_to_hex(load_address));

        // dump string table
        let string_table_offset = read_u32_at(&mut input, 0x1C).unwrap_or_else(ouch);
        let symbol_table_offset = read_u32_at(&mut input, 0x20).unwrap_or_else(ouch);
        input.seek(SeekFrom::Start(string_table_offset as u64)).unwrap_or_else(ouch);
        println!("string table:");
        let mut i = 0;
        while input.stream_position().unwrap_or_else(ouch) < symbol_table_offset as u64 {
            let s = read_len_prefixed_str(&mut input).unwrap_or_else(ouch);
            println!("    {}: {:?}", u32_to_hex(i), s);
            i += 1;
        }

        // dump symbol table
        assert_eq!(input.stream_position().unwrap_or_else(ouch), symbol_table_offset as u64);
        println!("yep, there's a symbol table"); // coward
    }
}
