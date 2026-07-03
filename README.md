# humio

[![GitHub Pages Docs](https://img.shields.io/badge/docs-GitHub_Pages-blue.svg)](https://TRRT004.github.io/humio/humio/index.html)
[![Rust CI Status](https://github.com/TRRT004/humio/actions/workflows/rust.yml/badge.svg)](https://github.com/TRRT004/humio/actions)

A humanized input simulation library for Rust scripting. Provides realistic mouse movement, keyboard typing, and configurable failure injection with automatic recovery — designed to mimic human imperfection rather than robotic precision.

## Features

- **WindMouse path generation** — human-like curved mouse paths with inertia
- **Gaussian delay distributions** — natural timing variance for clicks and keystrokes
- **Hover overshoots** — configurable probability of slightly overshooting a target before correcting
- **Scroll inertia** — physics-inspired momentum decay on scroll events
- **Typing error simulation**:
  - `Typo` — random character replacement with backspace correction
  - `Transposition` — swapped adjacent characters (e.g. `teh` → `the`)
  - `DoubleTap` — key bounce/double-press with correction
- **Key combination failure injection**:
  - `MissedModifier` — missing a modifier key
  - `WrongKeyTap` — pressing the wrong target key
  - `ReleasedModifierEarly` — releasing a modifier before the target key
  - `ModifierStuck` — failing to release a modifier after the combination
- **Flexible failure API** — per-failure probabilities with custom recovery closures
- **Recursion guard** — configurable max nesting depth prevents runaway recovery chains
- **Custom chance calculators** — override probability computation per failure type
- **`MockDevice`** — an in-memory input recorder for unit testing

## Quick Start

```toml
[dependencies]
humio = { path = "../humio" }  # or git/crates.io when published
```

```rust
use humio::{HumanizedDevice, TargetArea, Point, ClickFailure};
use humio::PhysicalDevice;
use enigo::Button;

fn main() -> Result<(), String> {
    let device = PhysicalDevice::new()?;
    let mut dev = HumanizedDevice::new(device);

    // Move and click with default humanizer config (overshoot + misclick chances)
    let target = TargetArea::Rect {
        top_left: Point::new(100, 200),
        bottom_right: Point::new(300, 250),
        target: None,
        std_dev_x: None,
        std_dev_y: None,
    };

    // Standard click with built-in failure & recovery
    dev.click_area(&target, Button::Left, true)?;

    // Type text with natural typing errors
    dev.text_humanized("Hello, world!", true)?;

    Ok(())
}
```

## Custom Failure Recovery

```rust
use humio::{HumanizedDevice, TargetArea, Point, ClickFailure};

let mut failures = vec![
    (
        ClickFailure::Misclick,
        0.08, // 8% chance
        Box::new(|d: &mut HumanizedDevice<_>| {
            // Custom recovery: log, wait, or do something specific
            Ok(())
        }) as Box<dyn FnMut(&mut _) -> _>
    ),
];

dev.click_area_flexible(&target, Button::Left, &mut failures)?;
```

## Developer CLI (Documentation & Git Automation)

The crate includes a built-in developer utility to automatically compile the API documentation, stage modified files, commit them, and push them to the remote repository in one command.

Run it via Cargo:
```bash
cargo run --bin docgen -- [OPTIONS]
```

### Available Flags

- `-m`, `--message <MSG>`: Specifies a custom Git commit message. (Default: `"docs: update API documentation"`)
- `--no-push`: Stage and commit files locally but skip running `git push`.
- `-h`, `--help`: Prints the CLI help message detailing usage.

## Architecture

```
humio/
└── src/
    ├── bin/
    │   └── docgen.rs           # Documentation generation and Git automation CLI tool
    ├── lib.rs                  # Public API: traits (Mouse, Keyboard, InputDevice), Point, etc.
    ├── physical_device.rs      # Enigo-backed real hardware device
    ├── mock.rs                 # In-memory MockDevice for testing
    └── humanizer/
        ├── mod.rs
        ├── device.rs           # HumanizedDevice<D> wrapper, recursion guard
        ├── failures.rs         # Failure enums, HumanizerConfig, FailureChanceCalculator
        ├── mouse.rs            # move_to_area, click_area, click_area_flexible, scroll
        ├── keyboard.rs         # text_humanized, key_combination_humanized, key_combination_flexible
        ├── target_area.rs      # TargetArea (Point, Rect, Circle, Polygon) with Gaussian sampling
        ├── wind_mouse.rs       # WindMouse path algorithm
        └── delay.rs            # Gaussian delay sampling utilities
```

## License

MIT
