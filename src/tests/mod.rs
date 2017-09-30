use super::{EEPROM, EEPROMController};
use stm32_hal::flash::{Flash, FlashResult};
use std::mem::size_of;
use std::vec::Vec;
use std::cell::RefCell;

mod memdump;

// Fake linker variables
#[export_name = "_eeprom_start"] pub static EEPROM_START: u32 = 0;
#[export_name = "_page_size"] pub static PAGE_SIZE: u32 = 0;
#[export_name = "_eeprom_pages"] pub static EEPROM_PAGES: u32 = 0;

#[test]
pub fn used() {
    assert_eq!(0, EEPROM_START);
    assert_eq!(0, PAGE_SIZE);
    assert_eq!(0, EEPROM_PAGES);
}

struct MockFlash {
    flash_mem: RefCell<Vec<u16>>,
    page_size: usize,
    page_count: usize
}

// Emulate MCU flash memory & FLASH control registers
impl MockFlash {
    fn load(filename: &str, page_size: usize, page_count: usize) -> MockFlash {
        let size = page_size * page_count / size_of::<u16>();
        let flash_mem = memdump::read_dump(filename);

        assert_eq!(size, flash_mem.len());
        MockFlash {
            flash_mem: RefCell::new(flash_mem),
            page_size,
            page_count
        }
    }
}

impl Flash for MockFlash {
    fn is_locked(&self) -> bool { false }

    fn status(&self) -> FlashResult { Ok(()) }

    unsafe fn erase_page(&self, address: usize) -> FlashResult {
        let mut vec = self.flash_mem.borrow_mut();

        let offset = address - (vec.as_ptr() as usize);
        assert_eq!(offset % self.page_size, 0);
        for i in 0..(self.page_size / 2) {
            vec[(offset / 2) + i] = 0xffff;
        }
        Ok(())
    }

    unsafe fn program_half_word(&self, address: usize, data: u16) -> FlashResult {
        let mut vec = self.flash_mem.borrow_mut();

        let offset = address - (vec.as_ptr() as usize);
        vec[offset / 2] = data;
        Ok(())
    }

    unsafe fn erase_all_pages(&self) -> FlashResult {
        unimplemented!()
    }

    unsafe fn lock(&self) { }

    unsafe fn unlock(&self) { }
}

impl <'a> EEPROM<'a> for MockFlash where MockFlash: 'a {
    fn eeprom(&'a self) -> EEPROMController<'a, Self> {
        EEPROMController::new(self.flash_mem.borrow().as_ptr() as usize, self.page_size, self.page_count, &self)
    }

    fn eeprom_params(&'a self, _first_page_address: usize, _page_size: usize, _page_count: usize) -> EEPROMController<'a, Self> {
        unimplemented!()
    }
}

fn test(initial: &str, expected: &str, cb: fn (&EEPROMController<MockFlash>)) {
    let mcu = MockFlash::load(initial, 1024, 2);
    let eeprom = mcu.eeprom();

    cb(&eeprom);

    let expected_file = memdump::read_file(expected);
    let expected: Vec<&str> = expected_file.lines().collect();
    let actual_dump = memdump::dump(&mcu.flash_mem.borrow(), mcu.page_size);
    let actual_lines: Vec<&str> = actual_dump.lines().collect();
    assert_eq!(expected, actual_lines);
}

fn test_init(initial: &str, expected: &str) {
    test(initial, expected, |eeprom|
        eeprom.init().unwrap())
}

fn test_erase(initial: &str, expected: &str) {
    test(initial, expected, |eeprom|
        eeprom.erase().unwrap())
}

// init() tests

#[test]
fn test_init_erased() { test_init("dumps/erased.txt", "dumps/empty.txt") }

#[test]
fn test_init_zeroed() { test_init("dumps/zeroed.txt", "dumps/empty.txt") }

#[test]
fn test_init_empty() { test_init("dumps/empty.txt", "dumps/empty.txt") }

#[test]
fn test_init_empty_page2() { test_init("dumps/empty-page2.txt", "dumps/empty-page2.txt") }

#[test]
fn test_init_two_empty_current() { test_init("dumps/two-empty-current-pages.txt", "dumps/empty.txt") }

#[test]
fn test_init_valid_simple() { test_init("dumps/valid-simple.txt", "dumps/valid-simple.txt") }

// Note that order is reversed when rescued (since we scan from the end)
#[test]
fn test_init_full_simple() { test_init("dumps/full-bogus.txt", "dumps/full-bogus.txt") }

#[test]
fn test_init_rescue_full_simple_duplicated() { test_init("dumps/full-bogus-duplicated-data.txt", "dumps/full-bogus-duplicated-data.txt") }


// erase() tests

#[test]
fn test_erase_empty() { test_erase("dumps/empty.txt", "dumps/empty.txt") }

#[test]
fn test_erase_empty_page2() { test_erase("dumps/empty-page2.txt", "dumps/empty.txt") }

#[test]
fn test_erase_simple() { test_erase("dumps/valid-simple.txt", "dumps/empty.txt") }

#[test]
fn test_erase_full_simple() { test_erase("dumps/full-bogus.txt", "dumps/empty.txt") }

// find() tests
#[test]
fn test_read_full_simple() {
    let mcu = MockFlash::load("dumps/full-bogus.txt", 1024, 2);
    let eeprom = mcu.eeprom();

    assert_eq!(0xdead, eeprom.read(1).unwrap()); // last item on the page
    assert_eq!(0xbeef, eeprom.read(2).unwrap());
    assert_eq!(true, eeprom.read(3).is_none());
}

// read() tests
#[test]
fn test_read_full_simple_duplicated() {
    let mcu = MockFlash::load("dumps/full-bogus-duplicated-data.txt", 1024, 2);
    let eeprom = mcu.eeprom();

    assert_eq!(0xdead, eeprom.read(1).unwrap());
    assert_eq!(0xbeef, eeprom.read(2).unwrap());
    assert_eq!(true, eeprom.read(3).is_none());
}

// write() tests
#[test]
fn test_write_empty() {
    test("dumps/empty.txt", "dumps/valid-simple.txt", |eeprom| {
        eeprom.write(1, 0xdead).unwrap();
        eeprom.write(2, 0xbeef).unwrap();
    });
}

#[test]
fn test_write_rescue() {
    test("dumps/full-bogus.txt", "dumps/valid-simple-third.txt", |eeprom| {
        eeprom.write(3, 0xacdb).unwrap();
    });
}

#[test]
fn test_write_rescue_duplicated() {
    test("dumps/full-simple.txt", "dumps/valid-simple-third.txt", |eeprom| {
        eeprom.write(3, 0xacdb).unwrap();
    });
}
