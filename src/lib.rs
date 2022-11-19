//! Flash-based EEPROM emulation for the STM32 series microcontrollers.
//! Uses 2 or more Flash pages for storing 16-bit data.
//!
//! # Examples
//! ```rust,no_run
//! use eeprom::EEPROM;
//! # use eeprom::{Flash,  EEPROMExt, Params};
//! # // Fake linker variables
//! # #[export_name = "_eeprom_start"] pub static EEPROM_START: u32 = 0;
//! # #[export_name = "_page_size"] pub static PAGE_SIZE: u32 = 0;
//! # #[export_name = "_eeprom_pages"] pub static EEPROM_PAGES: u32 = 0;
//! # pub fn main() {
//! struct MockFlash;
//! # impl <'a> EEPROMExt for &'a mut MockFlash {
//! #   fn eeprom(self, params: Params) -> eeprom::EEPROM<Self> { unimplemented!() }
//! # }
//! # impl <'a> Flash for &'a mut MockFlash {
//! # fn read(&mut self, params: &Params, offset: u32) -> Result<u16, ()> { unimplemented!() }
//! # fn write(&mut self, params: &Params, offset: u32, data: u16) -> Result<(), ()> { unimplemented!() }
//! # fn page_erase(&mut self, params: &Params, address: u32) -> Result<(), ()> { unimplemented!() }
//! # }
//! # let mut flash = MockFlash;
//! # let params = Params {
//! #   first_page: 0,
//! #   flash_size: 0,
//! #   page_size: 0,
//! #   page_count: 0
//! # };
//! // let param = Params { .. };
//! // let mut flash: stm32f1::stm32f103::FLASH = /* get flash somehow */;
//! let mut eeprom = flash.eeprom(params);
//! eeprom.init().expect("failed to init EEPROM");
//! eeprom.write(1, 0xdead).expect("failed to write data to EEPROM");
//! eeprom.write(2, 0xbeef).expect("failed to write data to EEPROM");
//! assert_eq!(0xdead, eeprom.read(1).unwrap());
//! assert_eq!(0xbeef, eeprom.read(2).unwrap());
//! assert_eq!(true, eeprom.read(3).is_none());
//! # }
//! ```
//!
//! # Panics
//! EEPROM controller will panic in the following cases:
//!
//! * No free space on the page even after compaction
//! * active page cannot be found during `read`/`write` operation (`init` makes sure that there
//!   is exactly one active page.
#![no_std]
#![warn(missing_docs)]
//#![deny(warnings)]

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(test)]
mod tests;

use core::mem::size_of;
use core::option::Option;
use core::result::Result;
#[cfg(feature = "stm32f103")]
use stm32f1xx_hal::flash::{Error as FlashError, FlashSize, Parts, SectorSize};

#[cfg(not(feature = "stm32f103"))]
type FlashError = ();
#[cfg(not(feature = "stm32f103"))]
type SectorSize = u32;
#[cfg(not(feature = "stm32f103"))]
type FlashSize = u32;

/// Result type for flash operations.
type FlashResult<T> = Result<T, FlashError>;

// STM32 allows programming half-words
type HalfWord = u16;
type Word = u32;

const ACTIVE_PAGE_MARKER: HalfWord = 0xABCD;
const ERASED_ITEM: Word = 0xffff_ffff; // two u16 half-words

// Each item is 16-bit tag plus 16-bit value
const ITEM_SIZE: u32 = size_of::<Word>() as u32;

/// EEPROM configuration parameters
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Index of the first page used as EEPROM
    pub first_page: u32,
    /// Total size of the flash
    pub flash_size: FlashSize,
    /// Size of the EEPROM page
    pub page_size: SectorSize,
    /// Count of EEPROM pages
    pub page_count: u32,
}

/// EEPROM-capable peripheral.
pub trait EEPROMExt
where
    Self: Sized,
{
    /// Create EEPROM controller with given configuration.
    fn eeprom(self, params: Params) -> EEPROM<Self>;
}

/// Low-level trait used by EEPROM implementation to access flash memory.
pub trait Flash {
    /// Read half-word (16-bit) value at a specified address. `address` must be an address of
    /// a location in the Flash memory aligned to two bytes.
    fn read(&mut self, params: &Params, offset: u32) -> FlashResult<HalfWord>;

    /// Write half-word (16-bit) value at a specified address. `address` must be an address of
    /// a location in the Flash memory aligned to two bytes.
    fn write(&mut self, params: &Params, offset: u32, data: u16) -> FlashResult<()>;

    /// Erase specified flash page. `address` must be an address of a beginning of the page in
    /// Flash memory.
    fn page_erase(&mut self, params: &Params, address: u32) -> FlashResult<()>;
}

#[cfg(feature = "stm32f103")]
impl<'a> EEPROMExt for &'a mut Parts {
    fn eeprom(self, params: Params) -> EEPROM<Self> {
        EEPROM::new(params, self)
    }
}

#[cfg(feature = "stm32f103")]
impl<'a> Flash for &'a mut Parts {
    fn read(&mut self, params: &Params, address: u32) -> FlashResult<HalfWord> {
        let writer = self.writer(params.page_size, params.flash_size);
        let data = writer.read(address, 2)?;
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    fn write(&mut self, params: &Params, address: u32, data: HalfWord) -> FlashResult<()> {
        let mut writer = self.writer(params.page_size, params.flash_size);
        writer.write(address, &data.to_le_bytes())?;
        Ok(())
    }

    fn page_erase(&mut self, params: &Params, address: u32) -> FlashResult<()> {
        let mut writer = self.writer(params.page_size, params.flash_size);
        writer.page_erase(address)?;
        Ok(())
    }
}

/// EEPROM controller. Uses Flash for implementing key-value storage for 16-bit data values.
pub struct EEPROM<F> {
    params: Params,
    // Amount of items per page (full words)
    page_items: u32,
    flash: F,
}

impl<F> EEPROM<F>
where
    F: Flash,
{
    /// Create new EEPROM controller.
    pub fn new(params: Params, flash: F) -> Self {
        EEPROM {
            params,
            page_items: (params.page_size as u32) * 1024 / ITEM_SIZE,
            flash,
        }
    }

    /// Initialize EEPROM controller. Checks that all internal data structures are in consistent
    /// state and fixes them otherwise.
    pub fn init(&mut self) -> FlashResult<()> {
        let active = self.find_active();
        for page in 0..self.params.page_count {
            match active {
                Some(p) if p == page => (), // Do not erase active page
                _ => {
                    self.erase_page(page)?;
                }
            }
        }

        if active.is_none() {
            // Active page not found, mark the first page as active
            return self.set_page_status(0, ACTIVE_PAGE_MARKER);
        }
        Ok(())
    }

    /// Erase all values stored in EEPROM
    pub fn erase(&mut self) -> FlashResult<()> {
        for page in 0..self.params.page_count {
            let start_offset =
                (self.params.first_page + page) * (self.params.page_size as u32) * 1024;
            self.flash.page_erase(&self.params, start_offset)?;
        }

        // Mark the first page as the active
        self.set_page_status(0, ACTIVE_PAGE_MARKER)
    }

    /// Read value for a specified tag
    ///
    /// # Panics
    /// * panics if active page cannot be found
    /// * panics if tag value has the most significant bit set to `1` (reserved value)
    pub fn read(&mut self, tag: HalfWord) -> Option<HalfWord> {
        assert_eq!(tag & 0b1000_0000_0000_0000, 0, "msb bit of `1` is reserved");

        let page = self.find_active().expect("cannot find active page");
        self.search(page, self.page_items, tag)
    }

    /// Write value for a specified tag.
    ///
    /// # Panics
    /// * panics if active page cannot be found
    /// * panics if page is full even after compacting it to the empty one
    /// * panics if tag value has the most significant bit set to `1` (reserved value)
    pub fn write(&mut self, tag: HalfWord, data: HalfWord) -> FlashResult<()> {
        assert_eq!(tag & 0b1000_0000_0000_0000, 0, "msb bit of `1` is reserved");

        let page = self.find_active().expect("cannot find active page");

        // rescue all the data to the free page first
        let page = self.rescue_if_full(page)?;

        for item in 1..self.page_items {
            if self.read_item(page, item) == ERASED_ITEM {
                return self.program_item(page, item, tag, data);
            }
        }
        panic!("too many variables");
    }

    fn rescue_if_full(&mut self, src_page: u32) -> Result<u32, FlashError> {
        // Check if last word of the page was written or not
        // Note that we check both data and the tag as in case of failure we might write
        // data, but not the tag.
        if self.read_item(src_page, self.page_items - 1) == ERASED_ITEM {
            // Page is not full yet -- last item is an erased value
            return Ok(src_page);
        }

        // Last word was not 0xffffffff, we need to rescue to the next page

        // Target page
        let tgt_page = if src_page == self.params.page_count - 1 {
            0
        } else {
            src_page + 1
        };
        let mut tgt_pos = 1; // skip page marker item

        // Start scanning source page from the end (to get the latest value)
        for item in (1..self.page_items).rev() {
            let (tag, data) = self.read_item_tuple(src_page, item);
            if tag == 0xffff {
                continue; // empty value -- skip
            }

            if self.search(tgt_page, tgt_pos, tag).is_none() {
                self.program_item(tgt_page, tgt_pos, tag, data)?;
                tgt_pos += 1;
            }
        }

        self.set_page_status(tgt_page, ACTIVE_PAGE_MARKER)?; // Mark target page as active
        self.erase_page(src_page)?; // Erase the source page

        Ok(tgt_page)
    }

    fn search(&mut self, page: u32, max_item: u32, tag: HalfWord) -> Option<HalfWord> {
        for item in (1..max_item).rev() {
            let (t, data) = self.read_item_tuple(page, item);
            if t == tag {
                return Some(data);
            }
        }
        None
    }

    fn find_active(&mut self) -> Option<u32> {
        (0..self.params.page_count).find(|&page| self.page_status(page) == ACTIVE_PAGE_MARKER)
    }

    fn page_status(&mut self, page: u32) -> HalfWord {
        let page_offset = self.page_offset(page);
        self.flash.read(&self.params, page_offset).unwrap()
    }

    fn set_page_status(&mut self, page: u32, status: HalfWord) -> FlashResult<()> {
        let page_offset = self.page_offset(page);
        self.flash.write(&self.params, page_offset, status)
    }

    fn page_offset(&self, page: u32) -> u32 {
        self.item_offset(page, 0)
    }

    fn item_offset(&self, page: u32, item: u32) -> u32 {
        debug_assert!(
            item < self.page_items,
            "item must be less than the amount of items per page"
        );
        debug_assert!(
            page < self.params.page_count,
            "page must be less than the amount of pages"
        );
        ((self.params.first_page + page) * self.page_items + item) * ITEM_SIZE
    }

    fn read_item(&mut self, page: u32, item: u32) -> Word {
        let offset = self.item_offset(page, item);
        let tag = self.flash.read(&self.params, offset).unwrap();
        let data = self.flash.read(&self.params, offset + 2).unwrap();
        (u32::from(data) << 16) + u32::from(tag)
    }

    fn read_item_tuple(&mut self, page: u32, item: u32) -> (HalfWord, HalfWord) {
        let item = self.read_item(page, item);
        ((item & 0xffff) as HalfWord, (item >> 16) as HalfWord)
    }

    fn erase_page(&mut self, page: u32) -> FlashResult<()> {
        if self.is_page_dirty(page) {
            let page_offset = self.page_offset(page);
            let result = self.flash.page_erase(&self.params, page_offset);
            debug_assert!(!self.is_page_dirty(page));
            result
        } else {
            Ok(())
        }
    }

    fn is_page_dirty(&mut self, page: u32) -> bool {
        for item in 0..self.page_items {
            let value = self.read_item(page, item);
            if value != ERASED_ITEM {
                return true;
            }
        }
        false
    }

    fn program_item(
        &mut self,
        page: u32,
        pos: u32,
        tag: HalfWord,
        data: HalfWord,
    ) -> FlashResult<()> {
        let item_addr = self.item_offset(page, pos);

        // Not found -- write the value first, so if we fail for whatever reason,
        // we don't have the default value of `0xffff` for the item with `tag`.
        self.flash.write(&self.params, item_addr + 2, data)?;
        self.flash.write(&self.params, item_addr, tag)?;
        Ok(())
    }
}
