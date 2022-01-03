use super::EEPROM;
use crate::{EEPROMExt, Flash, FlashResult, HalfWord, Params};
use std::mem::size_of;
use std::vec::Vec;

mod memdump;

// Fake linker variables
#[export_name = "_eeprom_start"]
pub static EEPROM_START: u32 = 0;
#[export_name = "_page_size"]
pub static PAGE_SIZE: u32 = 0;
#[export_name = "_eeprom_pages"]
pub static EEPROM_PAGES: u32 = 0;

#[test]
pub fn used() {
    assert_eq!(0, EEPROM_START);
    assert_eq!(0, PAGE_SIZE);
    assert_eq!(0, EEPROM_PAGES);
}

struct MockFlash {
    flash_mem: Vec<u16>,
    page_size: u32,
    page_count: u32,
}

// Emulate MCU flash memory & FLASH control registers
impl MockFlash {
    fn load(filename: &str, page_size: u32, page_count: u32) -> MockFlash {
        let size = page_size * page_count / (size_of::<u16>() as u32);
        let flash_mem = memdump::read_dump(filename);

        assert_eq!(size, flash_mem.len() as u32);
        MockFlash {
            flash_mem,
            page_size,
            page_count,
        }
    }
}

impl<'a> Flash for &'a mut MockFlash {
    fn read(&mut self, _params: &Params, offset: u32) -> FlashResult<HalfWord> {
        Ok(self.flash_mem[(offset / 2) as usize])
    }

    fn page_erase(&mut self, _params: &Params, offset: u32) -> FlashResult<()> {
        assert_eq!(offset % self.page_size, 0);
        for i in 0..(self.page_size / 2) {
            self.flash_mem[((offset / 2) + i) as usize] = 0xffff;
        }
        Ok(())
    }

    fn write(&mut self, _params: &Params, offset: u32, data: u16) -> FlashResult<()> {
        self.flash_mem[(offset / 2) as usize] = data;
        Ok(())
    }
}

impl<'a> EEPROMExt for &'a mut MockFlash {
    fn eeprom(self, config: Params) -> EEPROM<Self> {
        EEPROM::new(config, self)
    }
}

fn test(initial: &str, expected: &str, cb: for<'a> fn(&mut EEPROM<&'a mut MockFlash>)) {
    let mut mcu = MockFlash::load(initial, 1024, 2);
    let params = Params {
        first_page: 0,
        flash_size: 64 * 1024,
        page_size: 1,
        page_count: mcu.page_count,
    };
    let mut eeprom = mcu.eeprom(params);

    cb(&mut eeprom);

    let expected_file = memdump::read_file(expected);
    let expected: Vec<&str> = expected_file.lines().collect();
    let actual_dump = memdump::dump(&mcu.flash_mem, mcu.page_size);
    let actual_lines: Vec<&str> = actual_dump.lines().collect();
    assert_eq!(expected, actual_lines);
}

fn test_init(initial: &str, expected: &str) {
    test(initial, expected, |eeprom| eeprom.init().unwrap())
}

fn test_erase(initial: &str, expected: &str) {
    test(initial, expected, |eeprom| eeprom.erase().unwrap())
}

// init() tests

#[test]
fn test_init_erased() {
    test_init(
        "src/tests/test-data/erased.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_init_zeroed() {
    test_init(
        "src/tests/test-data/zeroed.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_init_empty() {
    test_init(
        "src/tests/test-data/empty.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_init_empty_page2() {
    test_init(
        "src/tests/test-data/empty-page2.txt",
        "src/tests/test-data/empty-page2.txt",
    )
}

#[test]
fn test_init_two_empty_current() {
    test_init(
        "src/tests/test-data/two-empty-current-pages.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_init_valid_simple() {
    test_init(
        "src/tests/test-data/valid-simple.txt",
        "src/tests/test-data/valid-simple.txt",
    )
}

// Note that order is reversed when rescued (since we scan from the end)
#[test]
fn test_init_full_simple() {
    test_init(
        "src/tests/test-data/full-bogus.txt",
        "src/tests/test-data/full-bogus.txt",
    )
}

#[test]
fn test_init_rescue_full_simple_duplicated() {
    test_init(
        "src/tests/test-data/full-bogus-duplicated-data.txt",
        "src/tests/test-data/full-bogus-duplicated-data.txt",
    )
}

// erase() tests

#[test]
fn test_erase_empty() {
    test_erase(
        "src/tests/test-data/empty.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_erase_empty_page2() {
    test_erase(
        "src/tests/test-data/empty-page2.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_erase_simple() {
    test_erase(
        "src/tests/test-data/valid-simple.txt",
        "src/tests/test-data/empty.txt",
    )
}

#[test]
fn test_erase_full_simple() {
    test_erase(
        "src/tests/test-data/full-bogus.txt",
        "src/tests/test-data/empty.txt",
    )
}

// find() tests
#[test]
fn test_read_full_simple() {
    let mut mcu = MockFlash::load("src/tests/test-data/full-bogus.txt", 1024, 2);
    let params = Params {
        first_page: 0,
        flash_size: 64 * 1024,
        page_size: 1,
        page_count: mcu.page_count,
    };
    let mut eeprom = mcu.eeprom(params);

    assert_eq!(0xdead, eeprom.read(1).unwrap()); // last item on the page
    assert_eq!(0xbeef, eeprom.read(2).unwrap());
    assert_eq!(true, eeprom.read(3).is_none());
}

// read() tests
#[test]
fn test_read_full_simple_duplicated() {
    let mut mcu = MockFlash::load(
        "src/tests/test-data/full-bogus-duplicated-data.txt",
        1024,
        2,
    );
    let params = Params {
        first_page: 0,
        flash_size: 64 * 1024,
        page_size: 1,
        page_count: mcu.page_count,
    };
    let mut eeprom = mcu.eeprom(params);

    assert_eq!(0xdead, eeprom.read(1).unwrap());
    assert_eq!(0xbeef, eeprom.read(2).unwrap());
    assert_eq!(true, eeprom.read(3).is_none());
}

// write() tests
#[test]
fn test_write_empty() {
    test(
        "src/tests/test-data/empty.txt",
        "src/tests/test-data/valid-simple.txt",
        |eeprom| {
            eeprom.write(1, 0xdead).unwrap();
            eeprom.write(2, 0xbeef).unwrap();
        },
    );
}

#[test]
fn test_write_rescue() {
    test(
        "src/tests/test-data/full-bogus.txt",
        "src/tests/test-data/valid-simple-third.txt",
        |eeprom| {
            eeprom.write(3, 0xacdb).unwrap();
        },
    );
}

#[test]
fn test_write_rescue_duplicated() {
    test(
        "src/tests/test-data/full-simple.txt",
        "src/tests/test-data/valid-simple-third.txt",
        |eeprom| {
            eeprom.write(3, 0xacdb).unwrap();
        },
    );
}
