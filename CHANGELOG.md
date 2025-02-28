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
