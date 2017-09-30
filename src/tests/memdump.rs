use std::fmt::Write;
use std::string::String;
use std::vec::Vec;
use std::mem::size_of;
use std::fs::File;
use std::io::Read;

pub fn dump(vec: &Vec<u16>, page_size: usize) -> String {
    let mut buf: String = String::new();
    for (page, p) in vec.chunks(page_size / size_of::<u16>()).enumerate() {
        if page != 0 {
            writeln!(buf).unwrap();
        }
        writeln!(buf, "Page: {}", page).unwrap();
        for (line, l) in p.chunks(16).enumerate() {
            write!(buf, "{: >3}: ", line * 8).unwrap();
            for (i, item) in l.chunks(2).enumerate() {
                if i != 0 {
                    write!(buf, " ").unwrap();
                }
                write!(buf, "{:0>4x}:{:0>4x}",
                       item.get(0).unwrap(),
                       item.get(1).unwrap()).unwrap();

            }
            writeln!(buf).unwrap();
        }
    }
    buf
}

// Loose parser for the format produced by `dump`
pub fn read(buf: &str) -> Vec<u16> {
    let mut mem: Vec<u16> = Vec::new();
    for line in buf.split("\n") {
        if let Some(':') = line.chars().nth(3) {
            for value in line.split_whitespace() {
                if let Some(':') = value.chars().nth(4) {
                    for data in value.split(':') {
                        mem.push(u16::from_str_radix(data, 16).unwrap());
                    }
                }
            }
        }
    }
    mem
}

pub fn read_file(filename: &str) -> String {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("failed to read mem file");
    contents
}

pub fn read_dump(filename: &str) -> Vec<u16> {
    read(&read_file(filename))
}