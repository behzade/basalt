use std::collections::HashMap;

use super::env::{Result, RuntimeError};
use super::runtime;
use super::value::{AllocationOwner, Value};

const DEFAULT_FRAME_BYTES: usize = 4 * 1024;
const DEFAULT_FRAME_OBJECTS: usize = 256;
const GLOBAL_USER_BYTES: usize = 64 * 1024;
const GLOBAL_USER_OBJECTS: usize = 1024;

#[derive(Debug)]
struct GlobalContext {
    owner: AllocationOwner,
    used_bytes: usize,
    object_count: usize,
    byte_limit: usize,
    object_limit: usize,
}

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
    parent: Option<u64>,
    kind: ContextKind,
    generation: u64,
    live: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextKind {
    RuntimeRoot,
    GlobalUser,
    Frame,
    NamedRegion,
}

#[derive(Debug)]
pub(crate) struct StackAllocator {
    frames: Vec<StackFrame>,
    regions: HashMap<u64, MemoryRegion>,
    active_region: Option<AllocationOwner>,
    next_context_id: u64,
    contexts: HashMap<u64, ContextState>,
    global: GlobalContext,
}

impl Default for StackAllocator {
    fn default() -> Self {
        let runtime_root_id = 1;
        let global_owner = AllocationOwner {
            id: 2,
            generation: 1,
        };
        let mut contexts = HashMap::new();
        contexts.insert(
            runtime_root_id,
            ContextState {
                name: "runtime root".to_string(),
                parent: None,
                kind: ContextKind::RuntimeRoot,
                generation: 1,
                live: true,
            },
        );
        contexts.insert(
            global_owner.id,
            ContextState {
                name: "global user context".to_string(),
                parent: Some(runtime_root_id),
                kind: ContextKind::GlobalUser,
                generation: global_owner.generation,
                live: true,
            },
        );
        Self {
            frames: vec![],
            regions: HashMap::new(),
            active_region: None,
            next_context_id: global_owner.id,
            contexts,
            global: GlobalContext {
                owner: global_owner,
                used_bytes: 0,
                object_count: 0,
                byte_limit: GLOBAL_USER_BYTES,
                object_limit: GLOBAL_USER_OBJECTS,
            },
        }
    }
}

impl StackAllocator {
    fn create_context(&mut self, name: String, parent: u64, kind: ContextKind) -> AllocationOwner {
        self.next_context_id += 1;
        let owner = AllocationOwner {
            id: self.next_context_id,
            generation: 1,
        };
        self.contexts.insert(
            owner.id,
            ContextState {
                name,
                parent: Some(parent),
                kind,
                generation: owner.generation,
                live: true,
            },
        );
        owner
    }

    pub(crate) fn return_target(&self) -> AllocationOwner {
        self.current_target().unwrap_or(self.global.owner)
    }

    pub(crate) fn define_region(
        &mut self,
        name: impl Into<String>,
        byte_limit: usize,
        object_limit: Option<usize>,
    ) -> Result<AllocationOwner> {
        let name = name.into();
        let parent = self.frames.last().map(|frame| frame.owner).ok_or_else(|| {
            RuntimeError(format!(
                "Memory region '{}' requires an active lexical context",
                name
            ))
        })?;
        let backing_ptr = runtime::alloc_bytes(byte_limit)?;
        let owner = self.create_context(
            format!("region {}", name),
            parent.id,
            ContextKind::NamedRegion,
        );
        self.regions.insert(
            owner.id,
            MemoryRegion::new(name, byte_limit, object_limit, backing_ptr, owner),
        );
        Ok(owner)
    }

    pub(crate) fn push_frame(&mut self, name: impl Into<String>) {
        let name = name.into();
        let parent = self
            .frames
            .last()
            .map(|frame| frame.owner.id)
            .unwrap_or(self.global.owner.id);
        let owner = self.create_context(format!("frame {}", name), parent, ContextKind::Frame);
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
        if let Some(region) = self.active_region {
            return Some(region);
        }
        self.frames.last().map(|frame| frame.owner)
    }

    pub(crate) fn alloc_value(&mut self, value: Value) -> Result<Value> {
        let target = self.current_target();
        self.alloc_value_in(target, value)
    }

    pub(crate) fn reset_region(&mut self, owner: AllocationOwner) -> Result<AllocationOwner> {
        let Some(region) = self.regions.get_mut(&owner.id) else {
            return Err(RuntimeError(
                "Unknown or destroyed memory region".to_string(),
            ));
        };
        if region.owner != owner {
            return Err(RuntimeError(format!(
                "Memory region '{}' handle has a stale generation",
                region.name
            )));
        }
        let Some(context) = self.contexts.get_mut(&region.owner.id) else {
            return Err(RuntimeError(format!(
                "Memory region '{}' has no allocation context",
                region.name
            )));
        };
        context.generation = context.generation.saturating_add(1);
        context.live = true;
        region.owner.generation = context.generation;
        region.used_bytes = 0;
        region.object_count = 0;
        Ok(region.owner)
    }

    pub(crate) fn destroy_region(&mut self, owner: AllocationOwner) -> Result<()> {
        let Some(region) = self.regions.remove(&owner.id) else {
            return Err(RuntimeError(
                "Unknown or already destroyed memory region".to_string(),
            ));
        };
        runtime::free_bytes(region.backing_ptr, region.byte_limit);
        if let Some(context) = self.contexts.get_mut(&owner.id) {
            context.live = false;
            context.generation = context.generation.saturating_add(1);
        }
        Ok(())
    }

    pub(crate) fn copy_value_to(
        &mut self,
        target: Option<AllocationOwner>,
        mut value: Value,
    ) -> Result<Value> {
        self.validate_value(&value)?;
        if value.stack_size_bytes() == 0 || value.owner() == target {
            return Ok(value);
        }
        if let Some(target) = target {
            value.detach_views_for_copy(target);
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
        let Some(target_context) = self.contexts.get(&target.id) else {
            return Err(RuntimeError(format!(
                "Unknown allocation context {} for {}",
                target.id,
                value.allocation_kind()
            )));
        };
        if !target_context.live || target_context.generation != target.generation {
            return Err(RuntimeError(format!(
                "Allocation context '{}' is no longer live for {}",
                target_context.name,
                value.allocation_kind()
            )));
        }
        if target_context.kind == ContextKind::RuntimeRoot {
            return Err(RuntimeError(format!(
                "Runtime root cannot store user {} values",
                value.allocation_kind()
            )));
        }
        if self
            .regions
            .get(&target.id)
            .is_some_and(|region| region.owner == target)
        {
            value = self.alloc_region_value(target, value, size)?;
            value.assign_owner_recursive(target);
            return Ok(value);
        }
        if target == self.global.owner {
            if self.global.object_count + 1 > self.global.object_limit {
                return Err(RuntimeError(format!(
                    "Global user context exhausted object limit: {}/{} objects before allocating {}",
                    self.global.object_count,
                    self.global.object_limit,
                    value.allocation_kind()
                )));
            }
            if self.global.used_bytes + size > self.global.byte_limit {
                return Err(RuntimeError(format!(
                    "Global user context exhausted memory: {}/{} bytes before allocating {} bytes for {}",
                    self.global.used_bytes,
                    self.global.byte_limit,
                    size,
                    value.allocation_kind()
                )));
            }
            self.global.object_count += 1;
            self.global.used_bytes += size;
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
                Some(context) if context.live && context.generation == owner.generation => {
                    let mut parent = context.parent;
                    while let Some(parent_id) = parent {
                        let Some(parent_context) = self.contexts.get(&parent_id) else {
                            error = Some(RuntimeError(format!(
                                "Allocation context '{}' has unknown parent {}",
                                context.name, parent_id
                            )));
                            return;
                        };
                        if !parent_context.live {
                            error = Some(RuntimeError(format!(
                                "Value's ancestor allocation context '{}' is no longer live",
                                parent_context.name
                            )));
                            return;
                        }
                        parent = parent_context.parent;
                    }
                }
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
        owner: AllocationOwner,
        value: Value,
        size: usize,
    ) -> Result<Value> {
        let Some(region) = self.regions.get_mut(&owner.id) else {
            return Err(RuntimeError(format!(
                "Unknown memory region for {} allocation",
                value.allocation_kind()
            )));
        };
        if region.owner != owner {
            return Err(RuntimeError(format!(
                "Memory region '{}' has a stale allocation generation",
                region.name
            )));
        }
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

    pub(crate) fn set_active_region(
        &mut self,
        region: Option<AllocationOwner>,
    ) -> Option<AllocationOwner> {
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
    previous_region: Option<AllocationOwner>,
}

impl<'a> AllocationRegionGuard<'a> {
    pub(crate) fn enter(
        stack: &'a std::cell::RefCell<StackAllocator>,
        region: Option<AllocationOwner>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::value::Managed;

    fn string(value: &str) -> Value {
        Value::Str(Managed::new(value.to_string()))
    }

    #[test]
    fn contexts_have_an_explicit_runtime_and_user_global_root() {
        let mut stack = StackAllocator::default();
        let global = stack.global.owner;
        let global_state = stack.contexts.get(&global.id).unwrap();
        let runtime_root_id = global_state.parent.unwrap();

        assert_eq!(global_state.kind, ContextKind::GlobalUser);
        assert_eq!(
            stack.contexts.get(&runtime_root_id).unwrap().kind,
            ContextKind::RuntimeRoot
        );

        stack.push_frame("main");
        let main = stack.frames.last().unwrap().owner;
        assert_eq!(
            stack.contexts.get(&main.id).unwrap().parent,
            Some(global.id)
        );
        let region = stack.define_region("Data", 64, Some(4)).unwrap();
        assert_eq!(
            stack.contexts.get(&region.id).unwrap().parent,
            Some(main.id)
        );
        stack.push_frame("callee");
        let callee = stack.frames.last().unwrap().owner;
        assert_eq!(
            stack.contexts.get(&callee.id).unwrap().parent,
            Some(main.id)
        );
    }

    #[test]
    fn ordinary_allocation_never_falls_back_to_global_memory() {
        let mut stack = StackAllocator::default();
        let error = stack.alloc_value(string("unscoped")).unwrap_err();
        assert_eq!(
            error.0,
            "No allocation context available for str".to_string()
        );
        assert_eq!(stack.global.object_count, 0);
    }

    #[test]
    fn outermost_return_uses_the_budgeted_user_global_context() {
        let mut stack = StackAllocator::default();
        let return_target = stack.return_target();
        assert_eq!(return_target, stack.global.owner);

        stack.push_frame("main");
        let local = stack.alloc_value(string("result")).unwrap();
        let result = stack.copy_value_to(Some(return_target), local).unwrap();
        stack.pop_frame();

        stack.validate_value(&result).unwrap();
        assert_eq!(result.owner(), Some(stack.global.owner));
        assert_eq!(stack.global.used_bytes, "result".len());
        assert_eq!(stack.global.object_count, 1);
    }

    #[test]
    fn runtime_root_cannot_store_user_values() {
        let mut stack = StackAllocator::default();
        let runtime_root_id = stack
            .contexts
            .get(&stack.global.owner.id)
            .unwrap()
            .parent
            .unwrap();
        let runtime_root = AllocationOwner {
            id: runtime_root_id,
            generation: 1,
        };

        stack.push_frame("main");
        let local = stack.alloc_value(string("result")).unwrap();
        let error = stack.copy_value_to(Some(runtime_root), local).unwrap_err();
        assert_eq!(error.0, "Runtime root cannot store user str values");
    }

    #[test]
    fn user_global_context_enforces_its_budget() {
        let mut stack = StackAllocator::default();
        stack.global.byte_limit = 3;
        let return_target = stack.return_target();

        stack.push_frame("main");
        let local = stack.alloc_value(string("four")).unwrap();
        let error = stack.copy_value_to(Some(return_target), local).unwrap_err();
        assert!(error.0.contains("Global user context exhausted memory"));
    }
}
