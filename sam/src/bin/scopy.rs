use sam::{ouch, read_u32_at};
use std::env;
use std::fs;
use std::io::{self, prelude::*, SeekFrom};

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert!(args.len() == 3);

    let mut input = fs::File::open(&args[1]).unwrap_or_else(ouch);
    let mut output = fs::File::create(&args[2]).unwrap_or_else(ouch);

    // check magic
    let mut buf = Vec::with_capacity(16);
    buf.resize(16, 0);
    input.read_exact(&mut buf).unwrap_or_else(ouch);
    assert_eq!(&buf, &[
        // magic = dc867b72-87f7-47da-a770-752af3299a3c
        0xdc, 0x86, 0x7b, 0x72, 0x87, 0xf7, 0x47, 0xda,
        0xa7, 0x70, 0x75, 0x2a, 0xf3, 0x29, 0x9a, 0x3c,
    ]);

    let code_and_data_offset = read_u32_at(&mut input, 0x18).unwrap_or_else(ouch);
    let string_table_offset = read_u32_at(&mut input, 0x1C).unwrap_or_else(ouch);
    let len = string_table_offset - code_and_data_offset;
    input.seek(SeekFrom::Start(code_and_data_offset as u64)).unwrap_or_else(ouch);
    io::copy(&mut input.take(len as u64), &mut output).unwrap_or_else(ouch);

    output.sync_data().unwrap();
}
