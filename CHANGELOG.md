# Changelog

## [0.2.2](https://github.com/debugger-bs/coreminer/compare/v0.2.1...v0.2.2)

### 📚 Documentation

- Fix typo in link of readme - ([d6ff688](https://github.com/debugger-bs/coreminer/commit/d6ff6889ae43ff798c09d76f899c5de132580661))


## [0.2.1](https://github.com/debugger-bs/coreminer/compare/v0.2.0...v0.2.1)

### 🐛 Bug Fixes

- *(cmserve)* Allow to use both example flags at once - ([48f6c05](https://github.com/debugger-bs/coreminer/commit/48f6c05badf103dd11e554139acc1360deb26560))
- Dont panic when dropping the breakpoint fails - ([ace3417](https://github.com/debugger-bs/coreminer/commit/ace34172f3dcdf3ed2c574a9ba26866a5e0586c7))

### 📚 Documentation

- Add section about cmserve to the readme - ([8a3af52](https://github.com/debugger-bs/coreminer/commit/8a3af5204a07238687f3e6c6246084370defddaf))

### ⚙️ Miscellaneous Tasks

- Update dummy compile scripts - ([185fb75](https://github.com/debugger-bs/coreminer/commit/185fb7584340aa48da2596fa6c99f905491508f5))
- Add how_many_fds.c example - ([f5c8beb](https://github.com/debugger-bs/coreminer/commit/f5c8bebb328d22f96c356982f6e50f425b883593))
- Remove the weird unreleased section from changelog - ([b452be1](https://github.com/debugger-bs/coreminer/commit/b452be1e8d90b940d0e5a63e01c4c0718ce74e2d))
- Run cargo ci on master but dont commit back - ([8a40169](https://github.com/debugger-bs/coreminer/commit/8a40169ae6a073297582410b44052a65656a3332))


## [Unreleased]

## [0.2.0](https://github.com/debugger-bs/coreminer/compare/v0.1.1...v0.2.0)

### ⛰️ Features

- *(basicui)* Update the help menu and set loglevel to info - ([fbf683a](https://github.com/debugger-bs/coreminer/commit/fbf683aec6d93694733a3ce9bb3cce71d727c45d))
- *(cmserve)* Update the help menu - ([ee41636](https://github.com/debugger-bs/coreminer/commit/ee41636bfc332831d6c71e36da77528a5d7804fc))
- *(cmserve)* Wrap feedback in a json object and add --example-feedbacks #21 - ([d19569c](https://github.com/debugger-bs/coreminer/commit/d19569c54db0d28e8a639375872de89285f74fb3))
- Setup human panic for the binaries - ([8fc32af](https://github.com/debugger-bs/coreminer/commit/8fc32afdb154f31e53bb1caa4615699a50222069))
- Show info for default executable if none was provided but run was used without args - ([3384231](https://github.com/debugger-bs/coreminer/commit/338423105eb9084b16d6db04c2f3525996de0f59))
- Add input struct for json interface #21 - ([5efc2b2](https://github.com/debugger-bs/coreminer/commit/5efc2b2ff48c155c65bfa7e8795a046e4b4705f0))
- Read json on \n #21 - ([0b7a785](https://github.com/debugger-bs/coreminer/commit/0b7a78582680bc57d74acf0ddaa8c7f5585b0189))
- Impl a basic JsonUI::process #16 - ([edf5640](https://github.com/debugger-bs/coreminer/commit/edf564018ff9ded655e9a446648b8c4f7ceb746c))
- Impl Deserialize for Status #20 #16 - ([03fa022](https://github.com/debugger-bs/coreminer/commit/03fa022abd86fc3a3fb339b82f5c2c06290bc31e))
- Make Status and Register Serialize #16 #20 - ([20197bd](https://github.com/debugger-bs/coreminer/commit/20197bd9c99b5523cb60cbc13e777ce014a91e6a))
- JsonUI::process outputs the serialized Feedback #19 #16 - ([3806b6c](https://github.com/debugger-bs/coreminer/commit/3806b6c73c872ac38a5dc9ebcc6653f6d15ceac7))
- Implement our own ProcessMemoryMap with Serialize #19 - ([9fa0bc4](https://github.com/debugger-bs/coreminer/commit/9fa0bc40e360a0b4f5af29ceb63961e25971372f))
- Make DebuggerError Serailize #19 - ([c9bed01](https://github.com/debugger-bs/coreminer/commit/c9bed01e86d7edd758bd7e4e797d48d1a14fd20b))
- Make OwnedSymbol Serialize #19 - ([707e4ad](https://github.com/debugger-bs/coreminer/commit/707e4ade402a71e531fedb95081a83bf9b56cc8a))
- Make Disassembly Serialize #19 - ([f210b05](https://github.com/debugger-bs/coreminer/commit/f210b0545137953df06d2b78082f9ec954cdd4a9))
- Make VariableValue Serialize #19 - ([bd14bca](https://github.com/debugger-bs/coreminer/commit/bd14bca6d99de52b3c0980c2cc17b16919c5024f))
- Make Stack Serialize #19 - ([f870110](https://github.com/debugger-bs/coreminer/commit/f8701108a635e2abd07ceb0cd29108ef44ec41dc))
- Replace libc::user_regs_struct with UserRegs #19 - ([97d5e60](https://github.com/debugger-bs/coreminer/commit/97d5e60f0b4d562436fac801c8e466d074e5b576))
- Make Backtrace Serialize #19 - ([9f39092](https://github.com/debugger-bs/coreminer/commit/9f39092d0a89e1c3b457a9a9108856c593733255))
- Add serde_json error - ([fe0c748](https://github.com/debugger-bs/coreminer/commit/fe0c7480c4a292fa723f5000385263771504bcdf))
- Make Addr Serialize - ([1cd0abc](https://github.com/debugger-bs/coreminer/commit/1cd0abc0994387ea339b7f216c10fcb435a0fccc))
- Add basic json interface - ([d4af0d2](https://github.com/debugger-bs/coreminer/commit/d4af0d2bb8eb92005a492bc54a830e04566cbd19))
- Add cmserve binary - ([0633171](https://github.com/debugger-bs/coreminer/commit/063317155070ed6738265ec11f50da667abc95c8))

### 🐛 Bug Fixes

- Fix many pedantic warnings and apply some in code - ([bebbc02](https://github.com/debugger-bs/coreminer/commit/bebbc02ee75f8668ad48e39923e29eb7d148f0f9))
- Json module was not declared - ([987e34d](https://github.com/debugger-bs/coreminer/commit/987e34d6177cd2a5c08d70132903c256f8b9ecbc))

### 🚜 Refactor

- Fix pedantic warnings - ([f34f45e](https://github.com/debugger-bs/coreminer/commit/f34f45e7db30214c25d3af2a21372816056ac54c))
- Setup the binaries with less verbose logging - ([4c012bd](https://github.com/debugger-bs/coreminer/commit/4c012bd37838ddc9f9e9db32a47f5f23de68a72a))
- JsonUI format_feedback is no longer a method - ([9e1dd81](https://github.com/debugger-bs/coreminer/commit/9e1dd81a0acd0fcfbcace3ff5bd8fc170a243b69))
- Cli build function checks if the given executable is okay, and remove the String field from the Executable errors - ([24c76dd](https://github.com/debugger-bs/coreminer/commit/24c76ddd5456f0be4902a018c78303bc96ac7aa9))
- Be more clear about when parse_datatype fails - ([86a1afd](https://github.com/debugger-bs/coreminer/commit/86a1afd62ef1a2c4a3830022933fa7ffa8a53a4a))
- Write_to_line now returns a result instead of panicing on error - ([487f2af](https://github.com/debugger-bs/coreminer/commit/487f2af1cf0110ac69032c6ec08ef7c0201b56e4))
- Remove unused Feedback variant Text - ([a039a9a](https://github.com/debugger-bs/coreminer/commit/a039a9a2bf034bdea91b96ff200b8807028169d3))
- Fix some pedantic warnings - ([229d063](https://github.com/debugger-bs/coreminer/commit/229d063128ad0fae689b89fbf70afcf4c6098254))
- Update Status struct for json interface #21 - ([2270042](https://github.com/debugger-bs/coreminer/commit/2270042413818c32ab612fcbb6d525045f05ef6c))

### 📚 Documentation

- Doctest was broken from mini-refactoring - ([3acb227](https://github.com/debugger-bs/coreminer/commit/3acb227cb4098b6d1f5b753d87e45f535acf441c))
- Fix doctests for --no-default-features - ([c87d58c](https://github.com/debugger-bs/coreminer/commit/c87d58c175caaacfecfb8523cb7e1fb9f052d32c))
- Document json.rs - ([33a2a7b](https://github.com/debugger-bs/coreminer/commit/33a2a7b7e34f0e320c242ab3451d9d343fd33031))
- Document cli.rs ui module - ([181fd22](https://github.com/debugger-bs/coreminer/commit/181fd22f8a963b6a477ff8b6c98d57639f79f7a7))
- Fix some examples - ([a3e09a7](https://github.com/debugger-bs/coreminer/commit/a3e09a7af789b7f93873f1e811250f5813a90d47))
- Document memorymap #21 - ([a4595d6](https://github.com/debugger-bs/coreminer/commit/a4595d6436469901a54cb428c526e9029bfa77ee))
- Update procmap documentation #21 - ([a68438b](https://github.com/debugger-bs/coreminer/commit/a68438b90f9b1df013a90febd05328d0f348b8c0))
- Document that OwnedSymbol skips some fields in serialize #21 - ([520d474](https://github.com/debugger-bs/coreminer/commit/520d474e3508c99c577b77925b03383ff146679b))
- Fix doctest for `get_process_map` #19 - ([2f7eeaa](https://github.com/debugger-bs/coreminer/commit/2f7eeaa05270f547a5d8c0a07b6acfa06b5a27c6))
- Format api docs in errors - ([e6101cb](https://github.com/debugger-bs/coreminer/commit/e6101cbc09428cb350a5eb41574ccf012193230c))

### 🧪 Testing

- Disassemble and serialize disassemble #24 - ([4fbd26b](https://github.com/debugger-bs/coreminer/commit/4fbd26bbca0af46c1fadb9afbe510bc2c1a73ece))
- OwnedSymbol serialization test #24 - ([fe62b6d](https://github.com/debugger-bs/coreminer/commit/fe62b6d71a787af931eaa2d2b6dfe243a591335b))
- Test_addr_serialize_deserialize for Addr #24 - ([c4cbbeb](https://github.com/debugger-bs/coreminer/commit/c4cbbeb58660db2c8deaa0b6fe31e7140f8a766a))

### ⚙️ Miscellaneous Tasks

- Configure release-plz - ([b722cf1](https://github.com/debugger-bs/coreminer/commit/b722cf156bd9897d35e18aee042e30e94acf252e))
- Dont run cargo ci on master (commit back is disallowed) - ([1ca92ea](https://github.com/debugger-bs/coreminer/commit/1ca92ea8ef15587fc74638c6f1b2fe16b32d3d1d))
- Automatic Rust CI changes - ([2d6df9e](https://github.com/debugger-bs/coreminer/commit/2d6df9e2f77e9b041063f262e8bca8cc04750f08))
- Add features to coreminer to keep things more organized - ([bac2dca](https://github.com/debugger-bs/coreminer/commit/bac2dca74ee283060a6743a6665dd08522062212))
- Ci now tests with --no-default-features too - ([c25d71d](https://github.com/debugger-bs/coreminer/commit/c25d71d693e7b5c4cdb7e9c1918d74cac3633252))
- Remove unused dependency addr2line - ([c8c3324](https://github.com/debugger-bs/coreminer/commit/c8c332443bd394c6fc19082650d586ab78222e40))
- Remove unused dependency ouroboros - ([4584ae0](https://github.com/debugger-bs/coreminer/commit/4584ae02855ba8e3c0d65c20d39ca2720a8e3e01))
- Disable some uneeded warnings - ([94ddd14](https://github.com/debugger-bs/coreminer/commit/94ddd14d077d9c334d8ab9d71f54f28fef189919))
- Make clippy super pedantic and comlplain about docs - ([94be3d8](https://github.com/debugger-bs/coreminer/commit/94be3d8c19977acfae5a4cf91550439e49c807bb))
- JsonUI::process is still a todo - ([e796b0a](https://github.com/debugger-bs/coreminer/commit/e796b0a56631825aaa12f310bd2f6407dffc52b5))
- Warn on missing docs - ([7e33980](https://github.com/debugger-bs/coreminer/commit/7e339806c28a07fa9bc4f978f0e9e525903c5ea8))

# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1](https://github.com/debugger-bs/coreminer/compare/v0.1.0...v0.1.1) - 2025-02-28

### Other

- change docs.rs links to go to the documentation directly
- change second emoji in readme because it's not displayed correctly on some devices (#14)
- fix github actions links in readme
- change links from PlexSheep/repo to organization
- set changelog

## [0.1.0] - 2025-02-26

### 🚀 Features

- Launch the debuggee
- Super basic debugger interface
- Feedback to ui
- Breakpoint struct
- Add breakpoints to debugee
- Breakpoints (setting) works
- Remove breakpoints
- Improve the basic cli interface with dialoguer
- Feedback error handling
- Set registers
- Rmem and wmem
- Step over breakpoint
- Get debug data from executable
- Process map
- Early disassemble
- Disassembly looks okay
- Disassembly datastructure
- Read any length from memory
- Write any length to process memory
- Set len for disassembly in debug-cli
- Debug symbols for functions
- Read which function we're in right now
- Find important constants
- Wait_signal
- Single step
- We think step over works
- Step over
- Only wait for interesting signals
- Add raw data to disassembly
- Don't allow stepping out of main if we know it
- Step into
- Step over
- Backtrace with libunwind
- Preparse all gimli symbols with a tree structure
- Query symbol tree
- Parse more symbol kinds
- Read types from dwarf ifno (just the usize)
- Get type for symbol
- Work on reading location expressions
- Eval expression maybe works
- We can read the location of an example
- Gimli frame_base parsing
- Impl custom debug for OwnedSymbol
- Read variable but wrong
- Stack datastructure
- Read stack
- Stop waiting on SIGWINCH
- Write variable with debug symbol
- Pass process maps to the ui
- Always check if the child is exited in functions that return a feedback
- Hide int3 instructions from disassembly (unless explicitly wished), add breakpoints to disassembly
- *(baseui)* Add default executable to base ui

### 🐛 Bug Fixes

- Create cstring with CString::new instead of from_str
- Some commands in the debug cli did not use get_number
- Step over breakpoint at cont
- Breakpoint inverse mask was wrong
- Log ignored signals and finish waiting on SIGILL
- Fix the step out SIGSEGV
- Log if go_back_step_over_bp actually does something
- Addresses for dwarf were wrongly parsed
- Addr debug didnt use hex
- Debug of addr had wrong format
- Stack addresses were displayed wrong
- Read variable reads an older version of the variable stored somewhere else???
- Read variable hack
- Wmem debug ui had wrong index
- Catch the exit status of the debuggee in wait_status
- Fill_to_const_arr did not use the internal vec
- Regs set parsing was broken in testing ui
- Set_bp and del_bp still used unwrap to get the debuggee
- Step_out used an unwrap to get the debuggee

### 🚜 Refactor

- Cli starts_with_any
- Move addr and add wmem+rmem
- The debuginfo loader
- Generalize debug symbols
- Impl Display for Disassembly
- Dse is here to stay (and maybe buggy)
- Use the gimli EntriesTree
- FrameInfo struct added
- Do not evaluate dwarf expressions at pre loading
- OwnedSymbol constructor change, read byte_size for types
- Rename parse_byte_site to parse_udata
- Error handling for variable reading logic
- Move addr to it's own module
- Addr module now works more with usize and has more traits
- Run any executable interactively
- Remove check_debuggee_status
- Remove unneeded fields and functions
- Variable access has less code duplication
- Entry_from_gimli is now much simpler without the large match
- Remove the prologue detection in step-in
- Remove the Addr::from_relative method, as it's just an addition
- Remove Addr::relative as it's just a subtraction
- Debuggee::get_symbol_by_offset does not panic when multiple matches are found, instead returns an error
- Remove old debug prints in run_debugger
- Remove unused method in dwarf_parse
- *(baseui)* Generally improve the baseui with error handling and a help menu

### 📚 Documentation

- Add a basic readme
- Api documentation for lib.rs
- Document the addr module
- Document the breakpoint module
- Document consts module
- Document the dbginfo module
- Document the debuggee module
- Amend enable and disable documentation of breakpoint with additional error reasons
- Document debugger module
- Fix doctests in debugger
- Document the disassemble module
- Fix a warning
- Ackowledge bugstalker for not just unwinding
- Document dwarf_parse module
- Remove example for private function
- Document errors module
- Document feedback module
- Document stack module
- Document the ui module
- Document unwind module
- Document the remaining core modules
- Adjust readme for changes to the baseui
- Add keywords and categories
- Fancy readme with logo and links
- Fix doctests, CliUi::build was broken

### 🧪 Testing

- Add tests for addr
- Tests for variablevalue
- Add test for stack operations
- Add tests for dbginfo

### ⚙️ Miscellaneous Tasks

- Setup basic project
- Add some deps which we probably need
- Automatic Rust CI changes
- Fix typo debugee -> debuggee
- Automatic Rust CI changes
- Remove uneeded part in cargo.yaml ci
- Add example dummy c script to debug
- Automatic Rust CI changes
- Build example dummy with debug info
- Add fixme to ptrace::step
- Automatic Rust CI changes
- Automatic Rust CI changes
- Automatic Rust CI changes
- Automatic Rust CI changes
- Automatic Rust CI changes
- Our debugger half works :)
- Automatic Rust CI changes
- Automatic Rust CI changes
- Better dummy compile scripts
- Install system deps in ci
- Automatic Rust CI changes
- Automatic Rust CI changes
- Automatic Rust CI changes
- Add dummy3.c
- Automatic Rust CI changes
- Automatic Rust CI changes
- Remove comment that is no longer relevant
- Rename a test in breakpoint
- Fix build-release script
- Rust ci now runs the doctests
- Allow publishing of coreminer
- Add msrv
- Create empty CHANGELOG
- Enforce maximum keywords limit
- Setup git-cliff
- Setup dependabot for cargo

<!-- generated by git-cliff -->
