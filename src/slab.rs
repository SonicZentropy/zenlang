use std::cell::UnsafeCell;

/// A generation-tagged slot used to detect stale handles (for weak refs).
struct Slot<T> {
    generation: u32,
    occupied: bool,
    value: UnsafeCell<Option<T>>,
}

/// A handle to an object stored in a [`Slab`]. The generation guards against
/// use-after-free when a slot is reused for a different object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle {
    pub index: u32,
    pub generation: u32,
}

impl Handle {
    pub fn null() -> Self {
        Handle { index: u32::MAX, generation: 0 }
    }

    pub fn is_null(&self) -> bool {
        self.index == u32::MAX
    }
}

/// A slab allocator that provides stable integer handles into a contiguous
/// store. Objects are accessed via [`Handle`] and can be mutated through
/// shared references using interior mutability (`UnsafeCell`).
///
/// This is safe because the VM is single-threaded and access is serialized.
pub struct Slab<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    generation_counter: u32,
}

impl<T> Slab<T> {
    pub fn new() -> Self {
        Self { slots: Vec::new(), free: Vec::new(), generation_counter: 1 }
    }

    pub fn insert(&mut self, value: T) -> Handle {
        if let Some(idx) = self.free.pop() {
            let slot = &mut self.slots[idx as usize];
            slot.generation = self.generation_counter;
            self.generation_counter += 1;
            slot.occupied = true;
            unsafe { *slot.value.get() = Some(value); }
            Handle { index: idx, generation: slot.generation }
        } else {
            let idx = self.slots.len() as u32;
            let generation = self.generation_counter;
            self.generation_counter += 1;
            self.slots.push(Slot {
                generation,
                occupied: true,
                value: UnsafeCell::new(Some(value)),
            });
            Handle { index: idx, generation }
        }
    }

    /// Read a value from the slab. Panics if the handle is stale or the slot is empty.
    pub fn get(&self, handle: Handle) -> &T {
        let slot = &self.slots[handle.index as usize];
        assert!(slot.occupied && slot.generation == handle.generation,
            "Slab::get: stale handle or empty slot");
        unsafe {
            (*slot.value.get()).as_ref().unwrap()
        }
    }

    /// Mutate a value in the slab through a shared reference.
    /// Safe because the VM is single-threaded.
    pub fn get_mut(&self, handle: Handle) -> &mut T {
        let slot = &self.slots[handle.index as usize];
        assert!(slot.occupied && slot.generation == handle.generation,
            "Slab::get_mut: stale handle or empty slot");
        unsafe {
            (*slot.value.get()).as_mut().unwrap()
        }
    }

    /// Remove a value from the slab, returning it. Returns `None` if the handle
    /// is stale or the slot was already empty.
    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        if (handle.index as usize) >= self.slots.len() {
            return None;
        }
        let slot = &mut self.slots[handle.index as usize];
        if slot.generation != handle.generation || !slot.occupied {
            return None;
        }
        slot.occupied = false;
        let value = unsafe { (*slot.value.get()).take() };
        self.free.push(handle.index);
        value
    }

    /// Check if a handle is still valid.
    pub fn is_valid(&self, handle: Handle) -> bool {
        if (handle.index as usize) >= self.slots.len() {
            return false;
        }
        let slot = &self.slots[handle.index as usize];
        slot.occupied && slot.generation == handle.generation
    }

    /// Get the number of occupied slots.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.generation_counter = 1;
    }

    pub fn iter_handles(&self) -> impl Iterator<Item = Handle> + '_ {
        self.slots.iter().enumerate().filter_map(move |(i, slot)| {
            if slot.occupied {
                Some(Handle { index: i as u32, generation: slot.generation })
            } else {
                None
            }
        })
    }
}

impl<T: Clone> Slab<T> {
    pub fn clone_all(&self) -> Vec<(Handle, T)> {
        self.iter_handles().map(|h| (h, self.get(h).clone())).collect()
    }
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self::new()
    }
}
