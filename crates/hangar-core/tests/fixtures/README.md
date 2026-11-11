# Fixtures

- `synthetic-*`: hand-written from the documented real formats (real probe output was not available). Used for cases that are hard to reproduce: second library, DLC, incomplete install, missing drive, UTF-8 names.
- Files without the prefix are real, masked output of `scripts/probe.ps1` (added later).

## Real fixtures
`real-*` files come from the user's machine via scripts/probe.ps1 (masked). Notable: real-loginusers.vdf has a single user and NO "MostRecent" key; real-installs.json has an entry with "install_dir": null; Steam librarycache uses the per-appid folder layout (<appid>/library_600x900.jpg).
