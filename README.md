[![crates.io](https://img.shields.io/crates/v/eeprom.svg)](https://crates.io/crates/eeprom)
[![crates.io](https://img.shields.io/crates/d/eeprom.svg)](https://crates.io/crates/eeprom)
[![CircleCI](https://img.shields.io/circleci/project/github/idubrov/eeprom.svg)](https://circleci.com/gh/idubrov/eeprom)
[![Codecov](https://img.shields.io/codecov/c/github/idubrov/eeprom.svg)](https://codecov.io/gh/idubrov/eeprom)

# eeprom

Flash-based EEPROM emulation for the STM32 series microcontrollers.
Uses 2 or more Flash pages for storing 16-bit data.

## Examples
```rust
use eeprom::EEPROM;
struct MockFlash;
// let param = Params { .. };
// let mut flash: stm32f1::stm32f103::FLASH = /* get flash somehow */;
let mut eeprom = flash.eeprom(params);
eeprom.init().expect("failed to init EEPROM");
eeprom.write(1, 0xdead).expect("failed to write data to EEPROM");
eeprom.write(2, 0xbeef).expect("failed to write data to EEPROM");
assert_eq!(0xdead, eeprom.read(1).unwrap());
assert_eq!(0xbeef, eeprom.read(2).unwrap());
assert_eq!(true, eeprom.read(3).is_none());
```

## Panics
EEPROM controller will panic in the following cases:

* No free space on the page even after compaction
* active page cannot be found during `read`/`write` operation (`init` makes sure that there
  is exactly one active page.

[Full Documentation](https://docs.rs/eeprom)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
