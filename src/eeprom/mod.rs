//! Flash-based EEPROM emulation for the STM32 series microcontrollers.
//! Uses 2 or more Flash pages for storing 16-bit data.
//!
//! # Examples
//! ```rust,no_run
//! extern crate stm32f103xx;
//! extern crate stm32_flash;
//! use stm32f103xx::FLASH;
//! # // Fake linker variables
//! # #[export_name = "_eeprom_start"] pub static EEPROM_START: u32 = 0;
//! # #[export_name = "_page_size"] pub static PAGE_SIZE: u32 = 0;
//! # #[export_name = "_eeprom_pages"] pub static EEPROM_PAGES: u32 = 0;
//! # pub fn main() {
//! # let flash = unsafe { &*FLASH.get() };
//! use stm32_flash::eeprom;
//! let eeprom = eeprom::default();
//! // Get flash somehow...
//! // let flash = FLASH.borrow(cs);
//! eeprom.init(&flash).expect("failed to init EEPROM");
//! eeprom.write(&flash, 1, 0xdead).expect("failed to write data to EEPROM");
//! eeprom.write(&flash, 2, 0xbeef).expect("failed to write data to EEPROM");
//! assert_eq!(0xdead, eeprom.read(1).unwrap());
//! assert_eq!(0xbeef, eeprom.read(2).unwrap());
//! assert_eq!(true, eeprom.read(3).is_none());
//! # }
//! ```
//!
//! # Panics
//! EEPROM controller will panic in the following cases:
//! * No free space on the page even after compaction
//! * active page cannot be found during `read`/`write` operation (`init` makes sure that there
//!   is exactly one active page.

#[cfg(test)]
mod tests;

use stm32f103xx::FLASH;
use core::option::Option;
use core::ptr;
use core::mem::size_of;

type HalfWord = u16; // STM32 allows programming half-words
type Word = u32;
type FlashResult = super::FlashResult;

const ACTIVE_PAGE_MARKER: HalfWord = 0xABCD;
const ERASED_ITEM: Word = 0xffff_ffff; // two u16 half-words
#[cfg(not(test))]
const FLASH_START: usize = 0x800_0000;

// Each item is 16-bit tag plus 16-bit value
const ITEM_SIZE: usize = size_of::<Word>();

// Default EEPROM (should be defined by the linker script, if feature is enabled)
#[cfg(feature = "default-eeprom")]
extern "C" {
    static _eeprom_start: u32;
    static _page_size: u32;
    static _eeprom_pages: u32;
}

/// EEPROM controller. Uses Flash for implementing key-value storage for 16-bit data values.
pub struct EEPROM {
    first_page_address: usize,
    // Amount of items per page (full words)
    page_items: usize,
    page_count: usize
}

/// Create default EEPROM controller. Uses variables defined by linker script to determine EEPROM location:
///  * `_eeprom_start` should be an address of the first page
///  * `_page_size` should be the FLASH page size (in bytes)
///  * `_eeprom_pages` should be the amount of FLASH pages to be used for EEPROM (2 is the minimum)
#[cfg(feature = "default-eeprom")]
pub fn default() -> EEPROM {
    let first_page_address = unsafe { &_eeprom_start } as *const u32 as usize;
    let page_size = unsafe { &_page_size } as *const u32 as usize;
    let page_count = unsafe { &_eeprom_pages } as *const u32 as usize;
    EEPROM {
        first_page_address,
        page_items: page_size / ITEM_SIZE,
        page_count
    }
}

/// Create EEPROM controller with given parameters:
///  * `first_page` should be an address of the first page to use for EEPROM
///  * `page_size` should be the page size (in bytes)
///  * `page_count` should be the amount of FLASH pages to be used for EEPROM (2 is the minimum)
pub fn new(first_page_address: usize, page_size: usize, page_count: usize) -> EEPROM {
    debug_assert!(page_count >= 2,
                  "EEPROM page count must be greater or equal to 2! Check your linker script for `_eeprom_pages`");
    debug_assert!((page_size & 0x3FF) == 0,
                  "EEPROM page size should be a multiple of 1K! Check your linker script for `_page_size`");
    // Tests fake FLASH memory
    #[cfg(not(test))]
    debug_assert!(((first_page_address - FLASH_START) % page_size) == 0,
                  "EEPROM first_page pointer does not point at the beginning of the FLASH page");
    EEPROM {
        first_page_address,
        page_items: page_size / ITEM_SIZE,
        page_count
    }
}

impl EEPROM {
    /// Initialize EEPROM controller. Checks that all internal data structures are in consistent
    /// state and fixes them otherwise.
    pub fn init(&self, flash: &FLASH) -> FlashResult {
        let flash = super::unlock(flash)?;

        let active = self.find_active();
        for page in 0..self.page_count {
            match active {
                Some(p) if p == page => (), // Do not erase active page
                _ => {
                    self.erase_page(&*flash, page)?;
                }
            }
        }

        if let None = active {
            // Active page not found, mark the first page as active
            return self.set_page_status(&*flash, 0, ACTIVE_PAGE_MARKER);
        }
        Ok(())
    }

    /// Erase all values stored in EEPROM
    pub fn erase(&self, flash: &FLASH) -> FlashResult {
        let flash = super::unlock(flash)?;

        for page in 0..self.page_count {
            self.erase_page(&*flash, page)?;
        }

        // Mark the first page as the active
        self.set_page_status(&*flash, 0, ACTIVE_PAGE_MARKER)
    }

    /// Read value for a specified tag
    ///
    /// # Panics
    /// * panics if active page cannot be found
    pub fn read(&self, tag: HalfWord) -> Option<HalfWord> {
        let page = self.find_active().expect("cannot find active page");
        self.search(page, self.page_items, tag)
    }

    /// Write value for a specified tag.
    ///
    /// # Panics
    /// * panics if active page cannot be found
    /// * panics if page is full even after compacting it to the empty one
    pub fn write(&self, flash: &FLASH, tag: HalfWord, data: HalfWord) -> FlashResult {
        let page = self.find_active().expect("cannot find active page");

        // rescue all the data to the free page first
        let page = self.rescue_if_full(flash, page)?;

        for item in 1..self.page_items {
            if self.read_item(page, item) == ERASED_ITEM {
                return self.program_item(flash, page, item, tag, data)
            }
        }
        panic!("too many variables");
    }

    fn rescue_if_full(&self, flash: &FLASH, src_page: usize) -> Result<usize, super::FlashError> {
        // Check if last word of the page was written or not
        // Note that we check both data and the tag as in case of failure we might write
        // data, but not the tag.
        if self.read_item(src_page, self.page_items - 1) == ERASED_ITEM {
            // Page is not full yet -- last item is an erased value
            return Ok(src_page);
        }

        // Last word was not 0xffffffff, we need to rescue to the next page

        // Target page
        let tgt_page = if src_page == self.page_count - 1 { 0 } else { src_page + 1 };
        let mut tgt_pos = 1; // skip page marker item

        // Start scanning source page from the end (to get the latest value)
        for item in (1..self.page_items).rev() {
            let (tag, data) = self.read_item_tuple(src_page, item);
            if tag == 0xffff {
                continue; // empty value -- skip
            }

            if let None = self.search(tgt_page, tgt_pos, tag) {
                self.program_item(flash, tgt_page, tgt_pos, tag, data)?;
                tgt_pos += 1;
            }
        }

        self.set_page_status(flash, tgt_page, ACTIVE_PAGE_MARKER)?; // Mark target page as active
        self.erase_page(flash, src_page)?; // Erase the source page

        Ok(tgt_page)
    }

    fn search(&self, page: usize, max_item: usize, tag: HalfWord) -> Option<HalfWord> {
        for item in (1..max_item).rev() {
            let (t, data) = self.read_item_tuple(page, item);
            if t == tag {
                return Some(data);
            }
        }
        None
    }

    fn find_active(&self) -> Option<usize> {
        for page in 0..self.page_count {
            if self.page_status(page) == ACTIVE_PAGE_MARKER {
                return Some(page);
            }
        }
        return None;
    }

    fn page_status(&self, page: usize) -> HalfWord {
        unsafe { ptr::read(self.page_address(page) as *mut HalfWord) }
    }

    fn set_page_status(&self, flash: &FLASH, page: usize, status: HalfWord) -> FlashResult {
        unsafe { super::program_half_word(flash, self.page_address(page) as *mut HalfWord, status) }
    }

    fn page_address(&self, page: usize) -> usize {
        self.item_address(page, 0)
    }

    fn item_address(&self, page: usize, item: usize) -> usize {
        debug_assert!(item < self.page_items, "item must be less than the amount of items per page");
        debug_assert!(page < self.page_count, "page must be less than the amount of pages");
        self.first_page_address + (page * self.page_items + item) * ITEM_SIZE
    }

    fn read_item(&self, page: usize, item: usize) -> Word {
        unsafe { ptr::read(self.item_address(page, item) as *mut Word) }
    }

    fn read_item_tuple(&self, page: usize, item: usize) -> (HalfWord, HalfWord) {
        let item = self.read_item(page, item);
        ((item & 0xffff) as HalfWord, (item >> 16) as HalfWord)
    }

    fn erase_page(&self, flash: &FLASH, page: usize) -> FlashResult {
        if self.is_page_dirty(page) {
            let result = self.do_erase_page(flash, page);
            debug_assert!(!self.is_page_dirty(page));
            result
        } else {
            Ok(())
        }
    }

    #[cfg(not(test))]
    fn do_erase_page(&self, flash: &FLASH, page: usize) -> FlashResult {
        unsafe { super::erase_page(flash, self.page_address(page) as *mut HalfWord) }
    }

    // Fake variant used in tests -- simply writes 0xff in the whole page
    #[cfg(test)]
    fn do_erase_page(&self, _flash: &FLASH, page: usize) -> FlashResult {
        for item in 0..self.page_items {
            unsafe { ptr::write(self.item_address(page, item) as *mut Word, 0xffff_ffffu32) }
        }
        Ok(())
    }

    fn is_page_dirty(&self, page: usize) -> bool {
        for item in 0..self.page_items {
            let value = self.read_item(page, item);
            if value != ERASED_ITEM {
                return true;
            }
        }
        return false;
    }

    fn program_item(&self, flash: &FLASH, page: usize, pos: usize, tag: HalfWord, data: HalfWord) -> FlashResult {
        let item_addr = self.item_address(page, pos);
        unsafe {
            // Not found -- write the value first, so if we fail for whatever reason,
            // we don't have the default value of `0xffff` for the item with `tag`.
            super::program_half_word(flash, (item_addr + 2) as *mut HalfWord, data)?;
            super::program_half_word(flash, item_addr as *mut HalfWord, tag)
        }
    }
}