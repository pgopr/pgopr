## Generate a shell completion file

This tutorial will show you how to generate a shell completition file for [**pgopr**](https://github.com/pgopr/pgopr).

### Preface

This tutorial assumes that you have an installation of [**pgopr**](https://github.com/pgopr/pgopr).

See [install pgopr](./01_install_operator.md) for more detail.

### Generate a shell completion file

- Bash

```bash
./pgopr completion --type bash
```

- zsh

```bash
./pgopr completion --type zsh
```

command generates shell completion scripts for supported shells (like Bash, Zsh, Fish, etc.).
These scripts enable auto-completion for the pgopr CLI commands, improving usability in the terminal. The shell type must be specified using the --type or -t flag.
