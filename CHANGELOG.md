# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/debugger-bs/coreminer/releases/tag/v0.1.0) - 2025-02-26

### Added

- *(baseui)* add default executable to base ui
- hide int3 instructions from disassembly (unless explicitly wished), add breakpoints to disassembly
- always check if the child is exited in functions that return a feedback
- pass process maps to the ui
- write variable with debug symbol
- stop waiting on SIGWINCH
- read stack
- stack datastructure
- read variable but wrong
- impl custom debug for OwnedSymbol
- gimli frame_base parsing
- we can read the location of an example
- eval expression maybe works
- work on reading location expressions
- get type for symbol
- read types from dwarf ifno (just the usize)
- parse more symbol kinds
- query symbol tree
- preparse all gimli symbols with a tree structure
- backtrace with libunwind
- step over
- step into
- don't allow stepping out of main if we know it
- add raw data to disassembly
- only wait for interesting signals
- step over
- we think step over works
- single step
- wait_signal
- find important constants
- read which function we're in right now
- debug symbols for functions
- set len for disassembly in debug-cli
- write any length to process memory
- read any length from memory
- disassembly datastructure
- disassembly looks okay
- early disassemble
- process map
- get debug data from executable
- step over breakpoint
- rmem and wmem
- set registers
- feedback error handling
- improve the basic cli interface with dialoguer
- remove breakpoints
- breakpoints (setting) works
- add breakpoints to debugee
- breakpoint struct
- feedback to ui
- super basic debugger interface
- launch the debuggee

### Fixed

- step_out used an unwrap to get the debuggee
- set_bp and del_bp still used unwrap to get the debuggee
- regs set parsing was broken in testing ui
- fill_to_const_arr did not use the internal vec
- catch the exit status of the debuggee in wait_status
- wmem debug ui had wrong index
- read variable hack
- read variable reads an older version of the variable stored somewhere else???
- stack addresses were displayed wrong
- debug of addr had wrong format
- addr debug didnt use hex
- addresses for dwarf were wrongly parsed
- log if go_back_step_over_bp actually does something
- fix the step out SIGSEGV
- log ignored signals and finish waiting on SIGILL
- breakpoint inverse mask was wrong
- step over breakpoint at cont
- some commands in the debug cli did not use get_number
- create cstring with CString::new instead of from_str

### Other

- fix doctests, CliUi::build was broken
- fancy readme with logo and links
- add msrv
- allow publishing of coreminer
- add keywords and categories
- adjust readme for changes to the baseui
- *(baseui)* generally improve the baseui with error handling and a help menu
- document the remaining core modules
- document unwind module
- document the ui module
- document stack module
- document feedback module
- document errors module
- remove example for private function
- document dwarf_parse module
- remove unused method in dwarf_parse
- ackowledge bugstalker for not just unwinding
- rust ci now runs the doctests
- fix build-release script
- fix a warning
- document the disassemble module
- fix doctests in debugger
- document debugger module
- amend enable and disable documentation of breakpoint with additional error reasons
- remove old debug prints in run_debugger
- document the debuggee module
- Debuggee::get_symbol_by_offset does not panic when multiple matches are found, instead returns an error
- document the dbginfo module
- document consts module
- document the breakpoint module
- document the addr module
- remove Addr::relative as it's just a subtraction
- remove the Addr::from_relative method, as it's just an addition
- api documentation for lib.rs
- add a basic readme
- add tests for dbginfo
- add test for stack operations
- tests for variablevalue
- rename a test in breakpoint
- add tests for addr
- remove comment that is no longer relevant
- remove the prologue detection in step-in
- entry_from_gimli is now much simpler without the large match
- variable access has less code duplication
- remove unneeded fields and functions
- remove check_debuggee_status
- run any executable interactively
- automatic Rust CI changes
- add dummy3.c
- Merge branch 'feat/print-stack' of https://github.com/PlexSheep/coreminer into feat/print-stack
- addr module now works more with usize and has more traits
- move addr to it's own module
- automatic Rust CI changes
- error handling for variable reading logic
- rename parse_byte_site to parse_udata
- OwnedSymbol constructor change, read byte_size for types
- do not evaluate dwarf expressions at pre loading
- FrameInfo struct added
- use the gimli EntriesTree
- install system deps in ci
- dse is here to stay (and maybe buggy)
- better dummy compile scripts
- impl Display for Disassembly
- Merge branch 'feat/dbginfo' of https://github.com/PlexSheep/coreminer into feat/dbginfo
- our debugger half works :)
- generalize debug symbols
- automatic Rust CI changes
- automatic Rust CI changes
- Merge branch 'feat/disassemble' of https://github.com/PlexSheep/coreminer into feat/disassemble
- automatic Rust CI changes
- Merge branch 'feat/dbginfo' of https://github.com/PlexSheep/coreminer into feat/dbginfo
- the debuginfo loader
- add fixme to ptrace::step
- build example dummy with debug info
- add example dummy c script to debug
- remove uneeded part in cargo.yaml ci
- move addr and add wmem+rmem
- automatic Rust CI changes
- cli starts_with_any
- fix typo debugee -> debuggee
- automatic Rust CI changes
- add some deps which we probably need
- setup basic project
- Initial commit
