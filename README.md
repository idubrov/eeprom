[![crates.io](https://img.shields.io/crates/v/stm32-flash.svg)](https://crates.io/crates/stm32-flash)
[![crates.io](https://img.shields.io/crates/d/stm32-flash.svg)](https://crates.io/crates/stm32-flash)
[![CircleCI](https://img.shields.io/circleci/project/github/idubrov/stm32-flash.svg)](https://circleci.com/gh/idubrov/stm32-flash)
[![Codecov](https://img.shields.io/codecov/c/github/idubrov/stm32-flash.svg)](https://codecov.io/gh/idubrov/stm32-flash)

# stm32-flash

Library for in-application programming of the Flash memory on the STM32 series microcontrollers.

## Examples

Erasing flash memory page and writing some data to it:

```rust
extern crate stm32f103xx;
extern crate stm32_flash;
// Get flash somehow...
// let flash = FLASH.borrow(cs);
let flash = stm32_flash::unlock(flash).unwrap(); // Unlock Flash for writing
unsafe {
    stm32_flash::erase_page(&flash, 0x800_fc00).unwrap(); // last 1K page on a chip with 64K flash memory
    stm32_flash::program_half_word(&flash, 0x800_fc00, 0xcafe).unwrap();
    stm32_flash::program_half_word(&flash, 0x800_fc02, 0xbabe).unwrap();
}
```

Additionally, this library includes support for EEPROM emulation. See the `eeprom` module
documentation for more details.

Simple example of writing and reading data from EEPROM backed by Flash memory:

## Examples
Write variables to the EEPROM:

```rust
extern crate stm32f103xx;
extern crate stm32_flash;

use stm32_flash::eeprom;
let eeprom = eeprom::default();
// Get flash somehow...
// let flash = FLASH.borrow(cs);
eeprom.init(&flash).expect("failed to init EEPROM");
eeprom.write(&flash, 1, 0xdead).expect("failed to write data to EEPROM");
eeprom.write(&flash, 2, 0xbeef).expect("failed to write data to EEPROM");
assert_eq!(0xdead, eeprom.read(1).unwrap());
assert_eq!(0xbeef, eeprom.read(2).unwrap());
assert_eq!(true, eeprom.read(3).is_none());
```


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
