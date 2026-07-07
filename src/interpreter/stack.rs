use std::collections::HashMap;

use super::env::{Result, RuntimeError};
use super::runtime;
use super::value::Value;

const DEFAULT_FRAME_BYTES: usize = 4 * 1024;
const DEFAULT_FRAME_OBJECTS: usize = 256;

#[derive(Debug, Clone)]
struct StackFrame {
    name: String,
    used_bytes: usize,
    object_count: usize,
    byte_limit: usize,
    object_limit: usize,
}

impl StackFrame {
    fn new(name: String) -> Self {
        Self {
            name,
            used_bytes: 0,
            object_count: 0,
            byte_limit: DEFAULT_FRAME_BYTES,
            object_limit: DEFAULT_FRAME_OBJECTS,
        }
    }
}

#[derive(Debug)]
struct MemoryRegion {
    name: String,
    used_bytes: usize,
    object_count: usize,
    byte_limit: usize,
    object_limit: usize,
    backing_ptr: u64,
}

impl MemoryRegion {
    fn new(name: String, byte_limit: usize, object_limit: Option<usize>, backing_ptr: u64) -> Self {
        Self {
            name,
            used_bytes: 0,
            object_count: 0,
            byte_limit,
            object_limit: object_limit.unwrap_or(DEFAULT_FRAME_OBJECTS),
            backing_ptr,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StackAllocator {
    frames: Vec<StackFrame>,
    regions: HashMap<String, MemoryRegion>,
    active_region: Option<String>,
}

impl StackAllocator {
    pub(crate) fn define_region(
        &mut self,
        name: impl Into<String>,
        byte_limit: usize,
        object_limit: Option<usize>,
    ) -> Result<()> {
        let name = name.into();
        let backing_ptr = runtime::alloc_bytes(byte_limit)?;
        self.regions.insert(
            name.clone(),
            MemoryRegion::new(name, byte_limit, object_limit, backing_ptr),
        );
        Ok(())
    }

    pub(crate) fn push_frame(&mut self, name: impl Into<String>) {
        self.frames.push(StackFrame::new(name.into()));
    }

    pub(crate) fn pop_frame(&mut self) {
        let _ = self.frames.pop();
    }

    pub(crate) fn alloc_value(&mut self, value: Value) -> Result<Value> {
        let size = value.stack_size_bytes();
        if size == 0 {
            return Ok(value);
        }
        if let Some(region_name) = self.active_region.clone() {
            return self.alloc_region_value(region_name, value, size);
        }
        let Some(frame) = self.frames.last_mut() else {
            return Err(RuntimeError(format!(
                "No stack frame available for {} allocation",
                value.allocation_kind()
            )));
        };
        if frame.object_count + 1 > frame.object_limit {
            return Err(RuntimeError(format!(
                "Stack frame '{}' exhausted object limit: {}/{} objects before allocating {}",
                frame.name,
                frame.object_count,
                frame.object_limit,
                value.allocation_kind()
            )));
        }
        if frame.used_bytes + size > frame.byte_limit {
            return Err(RuntimeError(format!(
                "Stack frame '{}' exhausted memory: {}/{} bytes before allocating {} bytes for {}",
                frame.name,
                frame.used_bytes,
                frame.byte_limit,
                size,
                value.allocation_kind()
            )));
        }
        frame.object_count += 1;
        frame.used_bytes += size;
        Ok(value)
    }

    fn alloc_region_value(
        &mut self,
        region_name: String,
        value: Value,
        size: usize,
    ) -> Result<Value> {
        let Some(region) = self.regions.get_mut(&region_name) else {
            return Err(RuntimeError(format!(
                "Unknown memory region '{}' for {} allocation",
                region_name,
                value.allocation_kind()
            )));
        };
        if region.object_count + 1 > region.object_limit {
            return Err(RuntimeError(format!(
                "Memory region '{}' exhausted object limit: {}/{} objects before allocating {}",
                region.name,
                region.object_count,
                region.object_limit,
                value.allocation_kind()
            )));
        }
        if region.used_bytes + size > region.byte_limit {
            return Err(RuntimeError(format!(
                "Memory region '{}' exhausted memory: {}/{} bytes before allocating {} bytes for {}",
                region.name,
                region.used_bytes,
                region.byte_limit,
                size,
                value.allocation_kind()
            )));
        }
        region.object_count += 1;
        region.used_bytes += size;
        Ok(value)
    }

    pub(crate) fn set_active_region(&mut self, region: Option<String>) -> Option<String> {
        std::mem::replace(&mut self.active_region, region)
    }
}

impl Drop for StackAllocator {
    fn drop(&mut self) {
        for region in self.regions.values() {
            runtime::free_bytes(region.backing_ptr, region.byte_limit);
        }
    }
}

pub(crate) struct StackFrameGuard<'a> {
    stack: &'a std::cell::RefCell<StackAllocator>,
}

impl<'a> StackFrameGuard<'a> {
    pub(crate) fn push(
        stack: &'a std::cell::RefCell<StackAllocator>,
        name: impl Into<String>,
    ) -> Self {
        stack.borrow_mut().push_frame(name);
        Self { stack }
    }
}

pub(crate) struct AllocationRegionGuard<'a> {
    stack: &'a std::cell::RefCell<StackAllocator>,
    previous_region: Option<String>,
}

impl<'a> AllocationRegionGuard<'a> {
    pub(crate) fn enter(
        stack: &'a std::cell::RefCell<StackAllocator>,
        region: Option<String>,
    ) -> Self {
        let previous_region = stack.borrow_mut().set_active_region(region);
        Self {
            stack,
            previous_region,
        }
    }
}

impl Drop for AllocationRegionGuard<'_> {
    fn drop(&mut self) {
        self.stack
            .borrow_mut()
            .set_active_region(self.previous_region.take());
    }
}

impl Drop for StackFrameGuard<'_> {
    fn drop(&mut self) {
        self.stack.borrow_mut().pop_frame();
    }
}
