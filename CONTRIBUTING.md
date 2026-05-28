# Contributing to Reactor.cloud

Thank you for your interest in contributing to Reactor.cloud! This document
provides guidelines for contributing to the project.

## Developer Certificate of Origin (DCO)

All contributions to this project must be signed off with the Developer
Certificate of Origin (DCO). By signing off your commits, you certify that
you wrote the code or have the right to submit it under the project's license.

### How to Sign Off

Add a `Signed-off-by` line to your commit message:

```
Signed-off-by: Your Name <your.email@example.com>
```

You can do this automatically with:

```bash
git commit -s -m "Your commit message"
```

Or configure git to always sign off:

```bash
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"
```

### The DCO

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.

Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

## How to Contribute

### Reporting Issues

- Search existing issues first to avoid duplicates
- Use the issue templates provided
- Include reproduction steps for bugs
- For security issues, see [SECURITY.md](SECURITY.md)

### Pull Requests

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Ensure tests pass: `cargo test --workspace`
5. Ensure code compiles: `cargo check --workspace`
6. Sign off your commits (see DCO section)
7. Open a pull request

### Code Style

- Follow Rust conventions and `rustfmt`
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Write tests for new functionality
- Keep commits focused and atomic

### Commit Messages

- Use the imperative mood ("Add feature" not "Added feature")
- Keep the first line under 72 characters
- Reference issues when applicable
- Always include the DCO sign-off

Example:

```
Add user invitation flow to reactor-auth

Implements the invitation API endpoints for multi-tenant organizations.
Includes email verification and role assignment.

Fixes #123

Signed-off-by: Your Name <your.email@example.com>
```

## License

By contributing, you agree that your contributions will be licensed under
the same license as the component you're contributing to:

- Most components: Apache License 2.0
- `reactor-cloud-api` and `reactor-ops`: BUSL 1.1

See [LICENSING.md](LICENSING.md) for the full license map.

## Questions?

Open a discussion on GitHub or reach out on Discord.
