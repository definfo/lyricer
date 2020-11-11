# Lyricer

Lyricer is a addon for waybar to display lyrics.

## Installation

Use `cargo` to build and install it.

```bash
cargo install --release lyricer
```
or

```bash
cargo build --release
```

## Usage

Add following lines to your `waybar` configuration:

```json
"modules-center": ["custom/lyrics"],
"custom/lyrics": {
    "format": "♪ {}",
    //"max-length": 15,
    "interval": 1, 
    "exec": "/usr/bin/cat /tmp/lyrics", 
    "exec-if": "test -f /tmp/lyrics",
    "return-type": "json"
}
```

## Contributing

Pull requests are welcome.

## License

[GPL3](https://choosealicense.com/licenses/gpl-3.0)