use super::super::eeprom;
use stm32f103xx::FLASH;
use std::mem::size_of;
use std::vec::Vec;

mod memdump;

const REG_SIZE: usize = size_of::<FLASH>();

struct FakeMCU {
    flash_mem: Vec<u16>,
    flash_reg: [u8; REG_SIZE],
    page_size: usize,
    page_count: usize
}

// Emulate MCU flash memory & FLASH control registers
impl FakeMCU {
    fn load(filename: &str, page_size: usize, page_count: usize) -> FakeMCU {
        let size = page_size * page_count / size_of::<u16>();
        let flash_mem = memdump::read_dump(filename);

        assert_eq!(size, flash_mem.len());
        FakeMCU {
            flash_mem,
            flash_reg: [0; REG_SIZE],
            page_size,
            page_count
        }
    }

    // Fake FLASH register
    fn flash_reg(&self) -> &'static FLASH {
        unsafe {
            let ptr = &self.flash_reg[0] as *const u8;
            &*(ptr as *mut FLASH)
        }
    }

    // Create an instance of the eeprom controller
    fn eeprom(&mut self) -> eeprom::EEPROM {
        eeprom::new(self.flash_mem.as_mut_ptr() as usize, self.page_size, self.page_count)
    }
}

fn test_init(initial: &str, expected: &str) {
    let mut mcu = FakeMCU::load(initial, 1024, 2);
    let eeprom = mcu.eeprom();

    eeprom.init(mcu.flash_reg()).unwrap();
    let expecte_file = memdump::read_file(expected);
    let expected: Vec<&str> = expecte_file.lines().collect();
    let actual_dump = memdump::dump(&mcu.flash_mem, mcu.page_size);
    let actual_lines: Vec<&str> = actual_dump.lines().collect();
    assert_eq!(expected, actual_lines);
}

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
fn test_init_rescue_full_simple() { test_init("dumps/full-simple.txt", "dumps/valid-simple.txt") }
