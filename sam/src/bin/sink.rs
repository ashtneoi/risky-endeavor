use sam::{
    ouch,
    Relocation,
    read_len_prefixed_str,
    read_u8,
    read_u16,
    read_u32,
    RelocationTable,
    StringTable,
    Symbol,
    SymbolTable,
    u32_to_hex,
};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, prelude::*, SeekFrom};

fn read_u32_at<F: Read + Seek>(mut f: F, pos: u64) -> io::Result<u32> {
    f.seek(SeekFrom::Start(pos))?;
    read_u32(f)
}

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert!(args.len() >= 2);

    for arg in &args[1..] {
        let mut input = fs::File::open(arg).unwrap_or_else(ouch);

        // check magic
        let mut buf = Vec::with_capacity(16);
        buf.resize(16, 0);
        input.read_exact(&mut buf).unwrap_or_else(ouch);
        assert_eq!(&buf, &[
            // magic = dc867b72-87f7-47da-a770-752af3299a3c
            0xdc, 0x86, 0x7b, 0x72, 0x87, 0xf7, 0x47, 0xda,
            0xa7, 0x70, 0x75, 0x2a, 0xf3, 0x29, 0x9a, 0x3c,
        ]);

        // check version
        let version = read_u8(&mut input).unwrap_or_else(ouch);
        assert_eq!(version, 0x00);

        // dump load address
        let load_address_offset = read_u32_at(&mut input, 0x14).unwrap_or_else(ouch);
        let load_address = read_u32_at(&mut input, load_address_offset as u64).unwrap_or_else(ouch);
        println!("load address = {}", u32_to_hex(load_address));

        // skip code and data

        // dump string table
        let string_table_offset = read_u32_at(&mut input, 0x1C).unwrap_or_else(ouch);
        let symbol_table_offset = read_u32_at(&mut input, 0x20).unwrap_or_else(ouch);
        input.seek(SeekFrom::Start(string_table_offset as u64)).unwrap_or_else(ouch);
        println!("string table:");
        let string_table = StringTable::deserialize(
            &mut input, symbol_table_offset - string_table_offset).unwrap_or_else(ouch);
        for &(offset, ref s) in &string_table.strings {
            println!("    {}: {:?}", u32_to_hex(offset), s);
        }
        assert_eq!(input.stream_position().unwrap_or_else(ouch), symbol_table_offset as u64);

        // dump symbol table
        let relocation_table_offset = read_u32_at(&mut input, 0x24).unwrap_or_else(ouch);
        input.seek(SeekFrom::Start(symbol_table_offset as u64)).unwrap_or_else(ouch);
        println!("symbol table:");
        let symbol_table = SymbolTable::deserialize(
            &mut input, relocation_table_offset - symbol_table_offset, &string_table
        ).unwrap_or_else(ouch);
        for symbol in &symbol_table.symbols {
            println!("    {}: {:?}", symbol.name(&string_table), symbol);
        }
        assert_eq!(input.stream_position().unwrap_or_else(ouch), relocation_table_offset as u64);

        // dump relocation table
        let file_end_offset = input.seek(SeekFrom::End(0)).unwrap_or_else(ouch) as u32;
        input.seek(SeekFrom::Start(relocation_table_offset as u64)).unwrap_or_else(ouch);
        println!("relocation table:");
        let relocation_table = RelocationTable::deserialize(
            &mut input, file_end_offset - relocation_table_offset
        ).unwrap_or_else(ouch);
        for reloc in &relocation_table.relocations {
            println!("    {}: {}, {:?}", u32_to_hex(reloc.offset), reloc.symbol(&symbol_table).name(&string_table), reloc.value);
        }
    }
}
