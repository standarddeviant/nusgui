# `nusgui`

`nusgui` is a rust GUI for communicating with BLE (Bluetooth Low Energy) devices that support the 'NUS' service to achieve serial port behavior.

## Installation

```bash
git clone https://github.com/standarddeviant/nusgui.git
cargo install --path .
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
- [ ] Auto save/load program state to file
- [ ] Input history that filters based on current, unsent text in input field
- [ ] Logging program events to file
- [ ] Logging NUS payloads to file with optional timestamps
- [ ] Scan result filtering

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License

[MIT](https://choosealicense.com/licenses/mit/)