//! # Debuggee Module
//!
//! Provides the core representation and management of a debugged process.
//!
//! This module contains the [`Debuggee`] struct, which represents a process being
//! debugged, and provides methods for interacting with that process. The debuggee
//! is controlled through the [ptrace] API and manages debug symbols, breakpoints,
//! memory access, and other low-level debugging operations.

use std::collections::HashMap;
use std::fmt::Display;

use gimli::{
    Attribute, DW_AT_frame_base, DW_AT_high_pc, DW_AT_location, DW_AT_low_pc, DW_AT_name,
    DW_AT_type, Unit,
};
use nix::sys::ptrace;
use nix::unistd::Pid;
use tracing::{debug, warn};

use crate::breakpoint::{Breakpoint, INT3_BYTE};
use crate::dbginfo::{search_through_symbols, CMDebugInfo, OwnedSymbol, SymbolKind};
use crate::disassemble::Disassembly;
use crate::dwarf_parse::GimliReaderThing;
use crate::errors::DebuggerError;
use crate::memorymap::ProcessMemoryMap;
use crate::stack::Stack;
use crate::{get_reg, mem_read_word, Result};
use crate::{mem_read, Addr};

/// Represents a process being debugged
///
/// The [`Debuggee`] struct is a central component of the coreminer debugger, representing
/// the target process that is being debugged. It manages breakpoints, accesses memory,
/// and provides methods for querying debug symbols.
pub struct Debuggee {
    /// Process ID of the debugged process
    pub(crate) pid: Pid,

    /// Map of active breakpoints by address
    pub(crate) breakpoints: HashMap<Addr, Breakpoint>,

    /// Debug symbols extracted from the executable
    pub(crate) symbols: Vec<OwnedSymbol>,
}

impl Debuggee {
    /// Creates a new debuggee instance from a process ID, debug info, and breakpoints
    ///
    /// # Parameters
    ///
    /// * `pid` - The process ID of the debugged process
    /// * `dbginfo` - Debug information extracted from the executable
    /// * `breakpoints` - Any initial breakpoints to set
    ///
    /// # Returns
    ///
    /// * `Ok(Debuggee)` - A new debuggee instance
    /// * `Err(DebuggerError)` - If the debuggee could not be created
    ///
    /// # Errors
    ///
    /// This function can fail if there are issues parsing the debug information
    /// or if the process cannot be accessed.
    pub(crate) fn build(
        pid: Pid,
        dbginfo: &CMDebugInfo<'_>,
        breakpoints: HashMap<Addr, Breakpoint>,
    ) -> Result<Self> {
        let mut symbols = Vec::new();
        let dwarf = &dbginfo.dwarf;
        let mut iter = dwarf.units();

        while let Some(header) = iter.next()? {
            let unit = dwarf.unit(header)?;
            let mut tree = unit.entries_tree(None)?;
            symbols.push(Self::process_tree(pid, dwarf, &unit, tree.root()?)?);
        }

        Ok(Self {
            pid,
            breakpoints,
            symbols,
        })
    }

    /// Terminates the debugged process
    ///
    /// Uses `PTRAC_KILL` to `SIGKILL` the debuggee process.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the process was successfully terminated
    /// * `Err(DebuggerError)` - If the process could not be terminated
    ///
    /// # Errors
    ///
    /// This function can fail if the ptrace kill operation fails.
    pub fn kill(&self) -> Result<()> {
        ptrace::kill(self.pid)?;
        Ok(())
    }

    /// Gets the memory map of a process by its PID
    ///
    /// # Parameters
    ///
    /// * `pid` - The process ID to query
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessMemoryMap)` - The memory map of the process
    /// * `Err(DebuggerError)` - If the memory map could not be retrieved
    ///
    /// # Errors
    ///
    /// This function can fail if the process's memory map cannot be accessed.
    fn get_process_map_by_pid(pid: Pid) -> Result<ProcessMemoryMap> {
        Ok(proc_maps::get_process_maps(pid.into())?.into())
    }

    /// Gets the base address of a process by its PID
    ///
    /// The base address is typically the starting address of the first mapping in
    /// the process's address space.
    ///
    /// # Parameters
    ///
    /// * `pid` - The process ID to query
    ///
    /// # Returns
    ///
    /// * `Ok(Addr)` - The base address of the process
    /// * `Err(DebuggerError)` - If the base address could not be determined
    ///
    /// # Errors
    ///
    /// This function can fail if the process's memory map cannot be accessed.
    pub fn get_base_addr_by_pid(pid: Pid) -> Result<Addr> {
        let process_map = Self::get_process_map_by_pid(pid)?;
        if process_map.regions.is_empty() {
            return Err(DebuggerError::NoDebugee);
        }

        // Get the start address of the first memory region
        Ok(process_map.regions[0].start_address)
    }

    /// Gets the memory map of the debugged process
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessMemoryMap)` - The memory map of the process
    /// * `Err(DebuggerError)` - If the memory map could not be retrieved
    ///
    /// # Errors
    ///
    /// This function can fail if the process's memory map cannot be accessed.
    #[inline]
    pub fn get_process_map(&self) -> Result<ProcessMemoryMap> {
        Self::get_process_map_by_pid(self.pid)
    }

    /// Gets the base address of the debugged process
    ///
    /// # Returns
    ///
    /// * `Ok(Addr)` - The base address of the process
    /// * `Err(DebuggerError)` - If the base address could not be determined
    ///
    /// # Errors
    ///
    /// This function can fail if the process's memory map cannot be accessed.
    pub fn get_base_addr(&self) -> Result<Addr> {
        Self::get_base_addr_by_pid(self.pid)
    }

    /// Disassembles a section of memory in the debugged process
    ///
    /// # Parameters
    ///
    /// * `addr` - The starting address to disassemble from
    /// * `len` - The number of bytes to disassemble
    /// * `literal` - Whether to show literal bytes (including breakpoint instructions)
    ///
    /// # Returns
    ///
    /// * `Ok(Disassembly)` - The disassembled code
    /// * `Err(DebuggerError)` - If the disassembly failed
    ///
    /// # Errors
    ///
    /// This function can fail if the memory cannot be read or if there are issues
    /// with the disassembly process.
    ///
    /// # Panics
    ///
    /// If a [Breakpoint] is enabled but has no saved data, this will panic.
    /// If a [Breakpoint] was found before making the [Disassembly], but the same breakpoint does
    /// not exist after the [Disassembly] was created, this will also panic.
    pub fn disassemble(&self, addr: Addr, len: usize, literal: bool) -> Result<Disassembly> {
        let mut data_raw: Vec<u8> = vec![0; len];
        mem_read(&mut data_raw, self.pid, addr)?;

        let mut bp_indexes = Vec::new();

        for (idx, byte) in data_raw.iter_mut().enumerate() {
            if *byte == INT3_BYTE {
                let bp = match self.breakpoints.get(&(addr + idx)) {
                    None => {
                        warn!(
                            "found an int3 without breakpoint at {}, ignoring",
                            addr + idx
                        );
                        continue;
                    }
                    Some(b) => b,
                };
                bp_indexes.push(idx);

                if !literal {
                    *byte = bp.saved_data().expect(
                        "breakpoint exists for a part of code that is an in3, but is disabled",
                    );
                }
            }
        }

        let out: Disassembly = Disassembly::disassemble(&data_raw, addr, &bp_indexes)?;

        for idx in bp_indexes {
            assert!(self.breakpoints.contains_key(&(addr + idx)), "a stored index that we thought had a breakpoint did not actually have a breakpoint");
            if !literal {
                data_raw[idx] = INT3_BYTE;
            }
        }

        Ok(out)
    }

    /// Creates an [`OwnedSymbol`] from a DWARF debugging information entry
    ///
    /// # Parameters
    ///
    /// * `pid` - The process ID (used to get the base address)
    /// * `dwarf` - The DWARF debug information
    /// * `unit` - The compilation unit containing the entry
    /// * `entry` - The debugging information entry
    ///
    /// # Returns
    ///
    /// * `Ok(OwnedSymbol)` - The parsed symbol
    /// * `Err(DebuggerError)` - If the symbol could not be parsed
    ///
    /// # Errors
    ///
    /// This function can fail if there are issues parsing the debug information
    /// or if required attributes are missing.
    fn entry_from_gimli(
        pid: Pid,
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        entry: &gimli::DebuggingInformationEntry<'_, '_, GimliReaderThing>,
    ) -> Result<OwnedSymbol> {
        let base_addr = Self::get_base_addr_by_pid(pid)?;

        let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
        let kind = SymbolKind::try_from(entry.tag())?;
        let low = Self::parse_addr_low(dwarf, unit, entry.attr(DW_AT_low_pc)?, base_addr)?;
        let high = Self::parse_addr_high(entry.attr(DW_AT_high_pc)?, low)?;
        let datatype: Option<usize> = Self::parse_datatype(entry.attr(DW_AT_type)?);
        let location: Option<Attribute<GimliReaderThing>> = entry.attr(DW_AT_location)?;
        let frame_base: Option<Attribute<GimliReaderThing>> = entry.attr(DW_AT_frame_base)?;

        let mut sym = OwnedSymbol::new(entry.offset().0, kind, &[], unit.encoding());
        sym.set_name(name);
        sym.set_location(location);
        sym.set_datatype(datatype);
        sym.set_low_addr(low);
        sym.set_high_addr(high);
        sym.set_frame_base(frame_base);
        Ok(sym)
    }

    /// Recursively processes a DWARF debug information tree
    ///
    /// # Parameters
    ///
    /// * `pid` - The process ID
    /// * `dwarf` - The DWARF debug information
    /// * `unit` - The compilation unit containing the tree
    /// * `node` - The tree node to process
    ///
    /// # Returns
    ///
    /// * `Ok(OwnedSymbol)` - The parsed symbol tree
    /// * `Err(DebuggerError)` - If the tree could not be processed
    ///
    /// # Errors
    ///
    /// This function can fail if there are issues parsing the debug information.
    fn process_tree(
        pid: Pid,
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        node: gimli::EntriesTreeNode<GimliReaderThing>,
    ) -> Result<OwnedSymbol> {
        let mut children: Vec<OwnedSymbol> = Vec::new();
        let mut parent = Self::entry_from_gimli(pid, dwarf, unit, node.entry())?;

        // then process it's children
        let mut children_tree = node.children();
        while let Some(child) = children_tree.next()? {
            // Recursively process a child.
            children.push(match Self::process_tree(pid, dwarf, unit, child) {
                Err(e) => {
                    debug!("could not parse a leaf of the debug symbol tree: {e}");
                    continue;
                }
                Ok(s) => s,
            });
        }

        parent.set_children(children);
        Ok(parent)
    }

    /// Gets symbols by name
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the symbol to find
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<OwnedSymbol>)` - The matching symbols
    /// * `Err(DebuggerError)` - If the symbols could not be retrieved
    ///
    /// # Errors
    ///
    /// This function cannot fail.
    pub fn get_symbol_by_name(&self, name: impl Display) -> Result<Vec<OwnedSymbol>> {
        let all: Vec<OwnedSymbol> = self
            .symbols_query(|a| a.name() == Some(&name.to_string()))
            .clone();

        Ok(all)
    }

    /// Gets a function symbol containing the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to find a function for
    ///
    /// # Returns
    ///
    /// * `Ok(Some(OwnedSymbol))` - The function symbol containing the address
    /// * `Ok(None)` - If no function contains the address
    /// * `Err(DebuggerError)` - If there was an error searching for functions
    ///
    /// # Errors
    ///
    /// This function cannot fail.
    pub fn get_function_by_addr(&self, addr: Addr) -> Result<Option<OwnedSymbol>> {
        debug!("get function for addr {addr}");
        for sym in self
            .symbols_query(|a| a.kind() == SymbolKind::Function)
            .iter()
            .cloned()
        {
            if sym.low_addr().is_some_and(|a| a <= addr)
                && sym.high_addr().is_some_and(|a| addr < a)
            {
                return Ok(Some(sym));
            }
        }

        Ok(None)
    }

    /// Gets local variables in scope at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to find local variables for
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<OwnedSymbol>)` - The local variables in scope
    /// * `Err(DebuggerError)` - If there was an error searching for variables
    ///
    /// # Errors
    ///
    /// This function cannot fail.
    pub fn get_local_variables(&self, addr: Addr) -> Result<Vec<OwnedSymbol>> {
        debug!("get locals of function {addr}");
        for sym in self.symbols_query(|a| a.kind() == SymbolKind::Function) {
            if sym.low_addr().is_some_and(|a| a <= addr)
                && sym.high_addr().is_some_and(|a| addr < a)
            {
                return Ok(sym.children().to_vec());
            }
        }

        Ok(Vec::new())
    }

    /// Gets a symbol by its DWARF offset
    ///
    /// # Parameters
    ///
    /// * `offset` - The DWARF offset of the symbol
    ///
    /// # Returns
    ///
    /// * `Ok(Some(OwnedSymbol))` - The symbol with the specified offset
    /// * `Ok(None)` - If no symbol has the specified offset
    /// * `Err(DebuggerError)` - If there was an error searching for symbols
    ///
    /// # Errors
    ///
    /// This function can fail if multiple items are found for that offset.
    pub fn get_symbol_by_offset(&self, offset: usize) -> Result<Option<OwnedSymbol>> {
        // FIXME: this might return multiple items if we're dealing with multiple
        // compilation units

        let v: Vec<OwnedSymbol> = self
            .symbols_query(|s| s.offset() == offset)
            .into_iter()
            .collect();
        if v.len() > 1 {
            return Err(crate::errors::DebuggerError::MultipleDwarfEntries);
        }
        if v.is_empty() {
            Ok(None)
        } else {
            Ok(Some(v[0].clone()))
        }
    }

    /// Gets the data type symbol for a given symbol
    ///
    /// # Parameters
    ///
    /// * `sym` - The symbol to get the data type for
    ///
    /// # Returns
    ///
    /// * `Ok(Some(OwnedSymbol))` - The data type symbol
    /// * `Ok(None)` - If the symbol has no data type
    /// * `Err(DebuggerError)` - If there was an error retrieving the data type
    ///
    /// # Errors
    ///
    /// This function can fail if [`Self::get_symbol_by_offset`] fails.
    #[inline]
    pub fn get_type_for_symbol(&self, sym: &OwnedSymbol) -> Result<Option<OwnedSymbol>> {
        if let Some(dt) = sym.datatype() {
            self.get_symbol_by_offset(dt)
        } else {
            Ok(None)
        }
    }

    /// Gets all debug symbols
    ///
    /// # Returns
    ///
    /// A slice containing all (root) debug symbols
    #[must_use]
    pub fn symbols(&self) -> &[OwnedSymbol] {
        &self.symbols
    }

    /// Searches through debug symbols recursively with a filter function
    ///
    /// # Parameters
    ///
    /// * `fil` - A filter function that returns true for symbols to include
    ///
    /// # Returns
    ///
    /// A vector of symbols, including children, that match the filter function
    pub fn symbols_query<F>(&self, fil: F) -> Vec<OwnedSymbol>
    where
        F: Fn(&OwnedSymbol) -> bool,
    {
        search_through_symbols(self.symbols(), fil)
    }

    /// Gets the current stack of the debugged process
    ///
    /// # Returns
    ///
    /// * `Ok(Stack)` - The current stack
    /// * `Err(DebuggerError)` - If the stack could not be retrieved
    ///
    /// # Errors
    ///
    /// This function can fail if the stack memory cannot be read or if the
    /// register values are not accessible.
    #[allow(clippy::similar_names)] // not my fault they named the registers that
    pub fn get_stack(&self) -> Result<Stack> {
        let rbp: Addr = get_reg(self.pid, crate::Register::rbp)?.into();
        let rsp: Addr = get_reg(self.pid, crate::Register::rsp)?.into();

        let mut next: Addr = rbp;
        let mut stack = Stack::new(rbp);
        while next >= rsp {
            stack.push(mem_read_word(self.pid, next)?);
            next -= 8usize;
        }

        Ok(stack)
    }
}
