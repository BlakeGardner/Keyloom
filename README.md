# Keyloom

Keyloom is a visual keyboard remapping application for Linux — think Karabiner-Elements for modern Linux, powered by [xremap](https://github.com/xremap/xremap).

See your keyboard, press or click a key, choose what it should do, and apply. Keyloom owns the user experience — layout detection, device management, application targeting, profiles, and service lifecycle — while xremap handles the low-level input remapping.

## Status

Early development. See [docs/Product_Plan.md](docs/Product_Plan.md) for the full product plan and [docs/First_Release_Scope.md](docs/First_Release_Scope.md) for the scope of the first release.

## Building

Keyloom is built with Rust and [libcosmic](https://github.com/pop-os/libcosmic). libcosmic runs on Wayland and X11 across most Linux distributions and desktop environments — it isn't limited to the COSMIC desktop.

```sh
cargo build
```

## License

TBD.
