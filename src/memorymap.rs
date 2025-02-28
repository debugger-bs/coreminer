use std::fmt::{self, Display};

use serde::Serialize;

use crate::addr::Addr;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryRegion {
    pub start_address: Addr,
    pub end_address: Addr,
    pub size: usize,
    pub permissions: MemoryPermissions,
    pub offset: usize,
    pub device: String,
    pub inode: usize,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub shared: bool,
    pub private: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessMemoryMap {
    pub regions: Vec<MemoryRegion>,
    pub total_mapped: usize,
    pub executable_regions: usize,
    pub writable_regions: usize,
    pub private_regions: usize,
}

impl From<Vec<proc_maps::MapRange>> for ProcessMemoryMap {
    fn from(ranges: Vec<proc_maps::MapRange>) -> Self {
        let regions: Vec<MemoryRegion> = ranges
            .iter()
            .map(|range| {
                // Correctly use MapRange API
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
