# `nusgui`

`nusgui` is a GUI application for communicating with BLE (Bluetooth Low Energy) devices that support the 'NUS' (Nordic UART Service) service to achieve serial port behavior.

`nusgui` is written in Rust and leverages `egui` for the GUI and `bluest` for the BLE functionality.
If you don't already have a device that implements the NUS BLE service, the *Seeed Studio XIAO nRF54L15* is an inexpensive device that can be programmed with the `shell_bt_nus` example project from NCS (nRF Connect SDK).

## Note on `async` vs. non-`async`
Well supported, cross-platform Rust BLE crates require `async` code. 
`egui` operations are not `async` since that would pause the main thread where the GUI is running.
To join the `async` BLE operations with the non-`async` GUI operations, we use
- `flume::unbounded()` channel for GUI -> BT communication
- `egui_inbox::UiInbox` for GUI <- BT communication

This 'threaded communication' requires the definition of 'threaded messages'. For simplicity all messages in either direction are of the type, `ThreadedNusMsg` which is a rust enum that can communicate 
- state (`Am...`) or 
- commands (`Do...`) or 
- data (`Data...`)

## Installation

Install from git clone:
```bash
git clone https://github.com/standarddeviant/nusgui.git
cd nusgui
cargo install --path .
```

Or download a binary for macOS or Windows from the releases.


### Linux install

If `cargo install --path .` fails due to lack of `pkg-config` configuration, cross-compiling with `cargo-zigbuild` may provide a workable solution.

To use `cargo-auditable` and `cargo-zigbuild` together, try this from the `nusgui` directory after cloning:
```bash
cargo install cargo-auditable cargo-zigbuild
cargo auditable zigbuild --release
cp ./target/release/nusgui ~/.cargo/bin
```

Or just `cargo-zigbuild` on its own
```bash
cargo install cargo-zigbuild
cargo zigbuild --release
cp ./target/release/nusgui ~/.cargo/bin
```

## Usage

To run the program run `nusgui` from a terminal.

To launch `nusgui` when issuing `ng` in a Powershell session, add this to the file located at `$PROFILE`:
```pwsh
function ng
{
  Start-Process -FilePath nusgui -WindowStyle Hidden
}
```

Starting `nusgui` using `-WindowStyle Hidden` will hide the terminal window when running `nusgui`.

The critical UI elements are:
- Light/Dark Toggle and Theme Selection
- Quit Button
- Before Connection
    - Start Scan Button
    - Stop Scan Button
    - Clickable Scan Results to Connect
- During Connection
    - Displayed Text from BLE device
    - Input Text to BLE device
    - Disconnect Button

## Desired Features
- [x] Auto save/load program state to file
- [x] Scan result filtering
- [ ] Input history that filters based on current, unsent text in input field
- [ ] Logging program events to file
- [ ] Logging NUS payloads to file with optional timestamps
- [ ] Support ANSI escape sequences for color support; leverage `egui_sgr` (RichText)

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

[MIT](https://choosealicense.com/licenses/mit/)
