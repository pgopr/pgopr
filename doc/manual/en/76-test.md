\newpage

## Testing

### All Tests

To run all project tests across all modules:

```bash
cargo test
```

### Run tests with output

To see debug output during tests:

```bash
cargo test -- --nocapture
```

---

### Formatting and Linting

Code formatting and linting are enforced by CI.

#### Format code

To automatically format your Rust source code:

```bash
cargo fmt --all
```

#### Run Clippy

To run the Rust linter and check for potential issues:

```bash
cargo clippy
```

All Clippy warnings should be resolved before submitting a pull request.
