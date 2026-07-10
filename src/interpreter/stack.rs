use std::collections::HashMap;

use super::env::{Result, RuntimeError};
use super::runtime;
use super::value::{AllocationOwner, Value};

const DEFAULT_FRAME_BYTES: usize = 4 * 1024;
const DEFAULT_FRAME_OBJECTS: usize = 256;

#[derive(Debug, Clone)]
struct StackFrame {
    owner: AllocationOwner,
    name: String,
    used_bytes: usize,
    object_count: usize,
    byte_limit: usize,
    object_limit: usize,
}

impl StackFrame {
    fn new(name: String, owner: AllocationOwner) -> Self {
        Self {
            owner,
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
    owner: AllocationOwner,
    name: String,
    used_bytes: usize,
    object_count: usize,
    byte_limit: usize,
    object_limit: usize,
    backing_ptr: u64,
}

impl MemoryRegion {
    fn new(
        name: String,
        byte_limit: usize,
        object_limit: Option<usize>,
        backing_ptr: u64,
        owner: AllocationOwner,
    ) -> Self {
        Self {
            owner,
            name,
            used_bytes: 0,
            object_count: 0,
            byte_limit,
            object_limit: object_limit.unwrap_or(DEFAULT_FRAME_OBJECTS),
            backing_ptr,
        }
    }
}

#[derive(Debug, Clone)]
struct ContextState {
    name: String,
    generation: u64,
    live: bool,
}

#[derive(Debug)]
pub(crate) struct StackAllocator {
    frames: Vec<StackFrame>,
    regions: HashMap<String, MemoryRegion>,
    active_region: Option<String>,
    next_context_id: u64,
    contexts: HashMap<u64, ContextState>,
    root_owner: AllocationOwner,
}

impl Default for StackAllocator {
    fn default() -> Self {
        let root_owner = AllocationOwner {
            id: 1,
            generation: 1,
        };
        let mut contexts = HashMap::new();
        contexts.insert(
            root_owner.id,
            ContextState {
                name: "program root".to_string(),
                generation: root_owner.generation,
                live: true,
            },
        );
        Self {
            frames: vec![],
            regions: HashMap::new(),
            active_region: None,
            next_context_id: root_owner.id,
            contexts,
            root_owner,
        }
    }
}

impl StackAllocator {
    fn create_context(&mut self, name: String) -> AllocationOwner {
        self.next_context_id += 1;
        let owner = AllocationOwner {
            id: self.next_context_id,
            generation: 1,
        };
        self.contexts.insert(
            owner.id,
            ContextState {
                name,
                generation: owner.generation,
                live: true,
            },
        );
        owner
    }

    pub(crate) fn define_region(
        &mut self,
        name: impl Into<String>,
        byte_limit: usize,
        object_limit: Option<usize>,
    ) -> Result<()> {
        let name = name.into();
        let backing_ptr = runtime::alloc_bytes(byte_limit)?;
        let owner = self.create_context(format!("region {}", name));
        self.regions.insert(
            name.clone(),
            MemoryRegion::new(name, byte_limit, object_limit, backing_ptr, owner),
        );
        Ok(())
    }

    pub(crate) fn push_frame(&mut self, name: impl Into<String>) {
        let name = name.into();
        let owner = self.create_context(format!("frame {}", name));
        self.frames.push(StackFrame::new(name, owner));
    }

    pub(crate) fn pop_frame(&mut self) {
        if let Some(frame) = self.frames.pop()
            && let Some(context) = self.contexts.get_mut(&frame.owner.id)
        {
            context.live = false;
            context.generation = context.generation.saturating_add(1);
        }
    }

    pub(crate) fn current_target(&self) -> Option<AllocationOwner> {
        if let Some(region) = &self.active_region {
            return self.regions.get(region).map(|region| region.owner);
        }
        Some(
            self.frames
                .last()
                .map(|frame| frame.owner)
                .unwrap_or(self.root_owner),
        )
    }

    pub(crate) fn alloc_value(&mut self, value: Value) -> Result<Value> {
        let target = self.current_target();
        self.alloc_value_in(target, value)
    }

    pub(crate) fn copy_value_to(
        &mut self,
        target: Option<AllocationOwner>,
        value: Value,
    ) -> Result<Value> {
        self.validate_value(&value)?;
        if value.stack_size_bytes() == 0 || value.owner() == target {
            return Ok(value);
        }
        self.alloc_value_in(target, value)
    }

    fn alloc_value_in(
        &mut self,
        target: Option<AllocationOwner>,
        mut value: Value,
    ) -> Result<Value> {
        let size = value.stack_size_bytes();
        if size == 0 {
            return Ok(value);
        }
        let Some(target) = target else {
            return Err(RuntimeError(format!(
                "No allocation context available for {}",
                value.allocation_kind()
            )));
        };
        if let Some(region_name) = self
            .regions
            .iter()
            .find_map(|(name, region)| (region.owner == target).then(|| name.clone()))
        {
            value = self.alloc_region_value(region_name, value, size)?;
            value.assign_owner_recursive(target);
            return Ok(value);
        }
        if target == self.root_owner {
            value.assign_owner_recursive(target);
            return Ok(value);
        }
        let Some(frame) = self.frames.iter_mut().find(|frame| frame.owner == target) else {
            return Err(RuntimeError(format!(
                "Allocation context is no longer live for {}",
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
        value.assign_owner_recursive(target);
        Ok(value)
    }

    pub(crate) fn validate_value(&self, value: &Value) -> Result<()> {
        let mut error = None;
        value.visit_owners(&mut |owner| {
            if error.is_some() {
                return;
            }
            match self.contexts.get(&owner.id) {
                Some(context) if context.live && context.generation == owner.generation => {}
                Some(context) => {
                    error = Some(RuntimeError(format!(
                        "Value outlived allocation context '{}' generation {}",
                        context.name, owner.generation
                    )));
                }
                None => {
                    error = Some(RuntimeError(format!(
                        "Value references unknown allocation context {}",
                        owner.id
                    )));
                }
            }
        });
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
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
