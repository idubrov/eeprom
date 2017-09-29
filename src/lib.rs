//! Library for in-application programming of the Flash area on the STM32 series microcontrollers.
#![no_std]
#![feature(const_fn)]
#![feature(const_size_of)]
#![warn(missing_docs)]
#![deny(warnings)]

pub mod eeprom;

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

extern crate stm32f103xx;
use core::ops::Deref;
use core::result::Result;

use stm32f103xx::FLASH;

const FLASH_KEY1: u32 = 0x4567_0123;
const FLASH_KEY2: u32 = 0xCDEF_89AB;

const ERASE_TIMEOUT: u32 = 0x000B_0000;
const PROGRAM_TIMEOUT: u32 = 0x0000_2000;

/// Flash operation error
#[derive(Copy, Clone, Debug)]
pub enum FlashError {
    /// Flash program and erase controller failed to unlock
    UnlockFailed,
    /// Timeout while waiting for the completion of the operation
    Timeout,
    /// Address to be programmed contains a value different from '0xFFFF' before programming
    ProgrammingError,
    /// Programming a write-protected address of the Flash memory
    WriteProtectionError,
    /// Programming and erase controller is busy
    Busy
}

/// A type alias for the result of a Flash operation.
pub type FlashResult = Result<(), FlashError>;

/// A type alias for the result of a Flash unlock method.
pub type UnlockResult<'a> = Result<UnlockGuard<'a>, FlashError>;

/// An RAII implementation of a "scoped unlock" of a Flash. When this structure is dropped (falls
/// out of scope), the Flash will be locked.
pub struct UnlockGuard<'a> {
    flash: &'a FLASH,
    should_lock: bool
}

impl<'a> Drop for UnlockGuard<'a> {
    fn drop(&mut self) {
        if self.should_lock {
            unsafe {
                raw_lock(self.flash);
            }
        }
    }
}

impl<'a> Deref for UnlockGuard<'a> {
    type Target = FLASH;

    fn deref(&self) -> &FLASH {
        self.flash
    }
}

/// Unlocks the Flash program and erase controller (FPEC).
/// An RAII guard is returned to allow scoped unlock of the Flash. When the guard goes out of scope,
/// the Flash will be unlocked.
///
/// # Note
/// Panics if flash is locked already.
pub fn unlock(flash: &FLASH) -> UnlockResult {
    let locked = is_locked(flash);
    if locked {
        unsafe {
            raw_unlock(flash)?;
        }
    }
    Ok(UnlockGuard { flash, should_lock: locked })
}

/// Unlocks the Flash program and erase controller (FPEC)
pub unsafe fn raw_unlock(flash: &FLASH) -> FlashResult {
    flash.keyr.write(|w| w.key().bits(FLASH_KEY1));
    flash.keyr.write(|w| w.key().bits(FLASH_KEY2));
    if is_locked(flash) {
        Err(FlashError::UnlockFailed)
    } else {
        Ok(())
    }
}

/// Locks the Flash program and erase controller (FPEC)
pub unsafe fn raw_lock(flash: &FLASH) {
    flash.cr.modify(|_, w| w.lock().set_bit());
}

/// Check if Flash program and erase controller is locked
pub fn is_locked(flash: &FLASH) -> bool {
    flash.cr.read().lock().bit_is_set()
}

/// Program half-word (16-bit) value at a specified address. `ptr` must point to an aligned
/// location in the Flash area.
pub unsafe fn program_half_word(flash: &FLASH, ptr: *mut u16, data: u16) -> FlashResult {
    status(flash)?;

    flash.cr.modify(|_, w| w.pg().set_bit());
    core::ptr::write(ptr, data); // Program the half-word
    let res = wait_complete(flash, PROGRAM_TIMEOUT);
    flash.cr.modify(|_, w| w.pg().clear_bit());
    res
}

/// Erase specified flash page. `ptr` must point at a beginning of the Flash page.
pub unsafe fn erase_page(flash: &FLASH, ptr: *mut u16) -> FlashResult {
    status(flash)?;

    flash.cr.modify(|_, w| w.per().set_bit());
    flash.ar.write(|w| w.bits(ptr as u32));
    flash.cr.modify(|_, w| w.strt().set_bit()); // Erase page
    let res = wait_complete(flash, ERASE_TIMEOUT);
    flash.cr.modify(|_, w| w.per().clear_bit());
    res
}

/// Erase all Flash pages
pub unsafe fn erase_all_pages(flash: &FLASH) -> FlashResult {
    status(flash)?;

    flash.cr.modify(|_, w| w.mer().set_bit());
    flash.cr.modify(|_, w| w.strt().set_bit()); // Erase all pages
    let res = wait_complete(flash, ERASE_TIMEOUT);
    flash.cr.modify(|_, w| w.mer().clear_bit());
    res
}

/// Wait till last Flash operation is complete and return Flash status.
fn wait_complete(flash: &FLASH, mut timeout: u32) -> FlashResult {
    while flash.sr.read().bsy().bit_is_set() && timeout > 0 {
        timeout -= 1
    }
    if timeout == 0 {
        return Err(FlashError::Timeout);
    }
    status(flash)
}

/// Check Flash status
pub fn status(flash: &FLASH) -> FlashResult {
    let sr = flash.sr.read();
    if sr.bsy().bit_is_set() {
        Err(FlashError::Busy)
    } else if sr.pgerr().bit_is_set() {
        Err(FlashError::ProgrammingError)
    } else if sr.wrprterr().bit_is_set() {
        Err(FlashError::WriteProtectionError)
    } else {
        Ok(())
    }
}
