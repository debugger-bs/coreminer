//! # Memory Map Module
//!
//! Provides functionality for analyzing memory maps of debugged processes.
//!
//! This module contains utilities for extracting and working with process memory maps,
//! which describe the layout of a process's virtual address space. Memory maps provide
//! information about which regions of memory are accessible to a process, their permissions,
//! and their mappings to files.
//!
//! The memory map information is extracted from the `/proc/<pid>/maps` file using the
//! [`proc_maps`] crate and provides a structured way to analyze process memory regions.

use std::fmt::{self, Display};

use serde::Serialize;

use crate::addr::Addr;

/// Represents a single region in a process's memory map
///
/// Each region corresponds to a line in the `/proc/<pid>/maps` file and represents
/// a contiguous range of memory with specific access permissions and backing.
///
/// # Examples
///
/// ```
/// use coreminer::memorymap::MemoryRegion;
/// use coreminer::memorymap::MemoryPermissions;
/// use coreminer::addr::Addr;
///
/// let region = MemoryRegion {
///     start_address: Addr::from(0x7f000000usize),
///     end_address: Addr::from(0x7f001000usize),
///     size: 0x1000,
///     permissions: MemoryPermissions {
///         read: true,
///         write: false,
///         execute: true,
///         shared: false,
///         private: true,
///     },
///     offset: 0,
///     device: "00:00".to_string(),
///     inode: 0,
///     path: Some("/lib/libc.so.6".to_string()),
/// };
///
/// assert_eq!(region.size, 0x1000);
/// assert!(region.permissions.read);
/// assert!(region.permissions.execute);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct MemoryRegion {
    /// Starting address of the memory region
    pub start_address: Addr,
    /// End address of the memory region (exclusive)
    pub end_address: Addr,
    /// Size of the memory region in bytes
    pub size: usize,
    /// Access permissions for the memory region
    pub permissions: MemoryPermissions,
    /// Offset within the mapped file (if any)
    pub offset: usize,
    /// Device identifier (major:minor)
    pub device: String,
    /// Inode number of the mapped file (0 for anonymous mappings)
    pub inode: usize,
    /// Path to the mapped file, if any
    pub path: Option<String>,
}

/// Represents a memory region's access permissions
///
/// Memory permissions determine how a process can access a particular memory region.
/// These permissions include read, write, execute, and private/shared flags.
///
/// # Examples
///
/// ```
/// use coreminer::memorymap::MemoryPermissions;
///
/// let perm = MemoryPermissions {
///     read: true,
///     write: false,
///     execute: true,
///     shared: false,
///     private: true,
/// };
///
/// assert!(perm.read);
/// assert!(!perm.write);
/// assert!(perm.execute);
/// assert!(perm.private);
/// assert!(!perm.shared);
/// ```
// TODO: consider consolidating private and shared
#[derive(Debug, Clone, Serialize)]
pub struct MemoryPermissions {
    /// Read permission
    pub read: bool,
    /// Write permission
    pub write: bool,
    /// Execute permission
    pub execute: bool,
    /// Whether the memory is shared
    pub shared: bool,
    /// Whether the memory is private
    pub private: bool,
}

/// Represents the complete memory map of a debugged process
///
/// A `ProcessMemoryMap` contains a collection of memory regions and summary statistics
/// about the process's memory usage.
///
/// # Examples
///
/// ```
/// use coreminer::memorymap::ProcessMemoryMap;
/// use proc_maps::get_process_maps;
///
/// // Get the memory map for the current process
/// let maps = get_process_maps(std::process::id() as i32)?;
/// let memory_map = ProcessMemoryMap::from(maps);
///
/// // Print the memory map summary
/// println!("Total mapped memory: {} bytes", memory_map.total_mapped);
/// println!("Number of regions: {}", memory_map.regions.len());
/// println!("Executable regions: {}", memory_map.executable_regions);
/// println!("Writable regions: {}", memory_map.writable_regions);
///
/// // Iterate through regions
/// for region in &memory_map.regions {
///     println!("Region at {:x} - {:x}: {} bytes",
///              region.start_address.usize(),
///              region.end_address.usize(),
///              region.size);
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct ProcessMemoryMap {
    /// List of memory regions in the process
    pub regions: Vec<MemoryRegion>,
    /// Total amount of mapped memory in bytes
    pub total_mapped: usize,
    /// Number of executable memory regions
    pub executable_regions: usize,
    /// Number of writable memory regions
    pub writable_regions: usize,
    /// Number of private memory regions
    pub private_regions: usize,
}

impl From<Vec<proc_maps::MapRange>> for ProcessMemoryMap {
    fn from(ranges: Vec<proc_maps::MapRange>) -> Self {
        let regions: Vec<MemoryRegion> = ranges
            .iter()
            .map(|range| {
                let start = range.start();
                let size = range.size();
                let end = start + size;

                // Determine shared/private from the flags (assuming 4th char is 'p' for private, 's' for shared)
                let is_private = range.flags.len() >= 4 && &range.flags[3..4] == "p";
                let is_shared = range.flags.len() >= 4 && &range.flags[3..4] == "s";

                MemoryRegion {
                    start_address: Addr::from(start),
                    end_address: Addr::from(end),
                    size,
                    permissions: MemoryPermissions {
                        read: range.is_read(),
                        write: range.is_write(),
                        execute: range.is_exec(),
                        shared: is_shared,
                        private: is_private,
                    },
                    offset: range.offset,
                    device: range.dev.clone(),
                    inode: range.inode,
                    path: range.filename().map(|p| p.to_string_lossy().to_string()),
                }
            })
            .collect();

        // Calculate summary statistics
        let total_mapped = regions.iter().map(|r| r.size).sum();
        let executable_regions = regions.iter().filter(|r| r.permissions.execute).count();
        let writable_regions = regions.iter().filter(|r| r.permissions.write).count();
        let private_regions = regions.iter().filter(|r| r.permissions.private).count();

        ProcessMemoryMap {
            regions,
            total_mapped,
            executable_regions,
            writable_regions,
            private_regions,
        }
    }
}

impl Display for ProcessMemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Process Memory Map:")?;
        writeln!(f, "Total regions: {}", self.regions.len())?;
        writeln!(f, "Total mapped: {} bytes", self.total_mapped)?;
        writeln!(f, "Executable regions: {}", self.executable_regions)?;
        writeln!(f, "Writable regions: {}", self.writable_regions)?;
        writeln!(f, "Private regions: {}", self.private_regions)?;

        for (i, region) in self.regions.iter().enumerate() {
            let perm_str = format!(
                "{}{}{}{}",
                if region.permissions.read { "r" } else { "-" },
                if region.permissions.write { "w" } else { "-" },
                if region.permissions.execute { "x" } else { "-" },
                if region.permissions.private {
                    "p"
                } else if region.permissions.shared {
                    "s"
                } else {
                    "-"
                },
            );

            writeln!(
                f,
                "#{}: {:016x}-{:016x} {} ({} bytes) {}",
                i,
                region.start_address.usize(),
                region.end_address.usize(),
                perm_str,
                region.size,
                region.path.as_deref().unwrap_or("[anonymous]")
            )?;
        }

        Ok(())
    }
}
