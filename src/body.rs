use cgmath::*;
use serde::{Deserialize, Serialize};
use std::{f64::consts::PI, num::NonZeroUsize, ptr::NonNull};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub name: String,
    pub pos: Vector2<f64>,
    pub vel: Vector2<f64>,
    pub radius: f64,
    pub density: f64,
    pub color: Vector3<f64>,
}

impl Body {
    pub fn mass(&self) -> f64 {
        self.density * PI * (self.radius * self.radius)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BodyId(NonZeroUsize);

impl BodyId {
    pub fn next_id() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static ID: AtomicUsize = AtomicUsize::new(1);
        let Some(next_id) = NonZeroUsize::new(ID.fetch_add(1, Ordering::Relaxed)) else {
            eprintln!("BodyId counter somehow overflow, exiting");
            std::process::abort()
        };
        Self(next_id)
    }

    pub fn get_id(&self) -> NonZeroUsize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct BodyList {
    bodies: Vec<(BodyId, Body)>,
}

impl BodyList {
    pub fn new() -> Self {
        Self { bodies: vec![] }
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn reserve(&mut self, additional: usize) {
        self.bodies.reserve(additional);
    }

    pub fn insert(&mut self, id: BodyId, body: Body) {
        match self.bodies.binary_search_by_key(&id, |&(id, _)| id) {
            Ok(_) => panic!("Tried to insert body {id:?} twice"),
            Err(index) => self.bodies.insert(index, (id, body)),
        }
    }

    pub fn push(&mut self, body: Body) -> BodyId {
        let id = BodyId::next_id();
        self.bodies.push((id, body));
        id
    }

    pub fn remove(&mut self, id: BodyId) -> Option<Body> {
        match self.bodies.binary_search_by_key(&id, |&(id, _)| id) {
            Ok(index) => Some(self.bodies.remove(index).1),
            Err(_) => None,
        }
    }

    pub fn get(&self, id: BodyId) -> Option<&Body> {
        match self.bodies.binary_search_by_key(&id, |&(id, _)| id) {
            Ok(index) => Some(&self.bodies[index].1),
            Err(_) => None,
        }
    }

    pub fn get_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        match self.bodies.binary_search_by_key(&id, |&(id, _)| id) {
            Ok(index) => Some(&mut self.bodies[index].1),
            Err(_) => None,
        }
    }

    pub fn get_disjoint_mut<const N: usize>(&mut self, ids: [BodyId; N]) -> [Option<&mut Body>; N] {
        self.maybe_get_disjoint_mut(ids.map(Some))
    }

    pub fn maybe_get_disjoint_mut<const N: usize>(
        &mut self,
        ids: [Option<BodyId>; N],
    ) -> [Option<&mut Body>; N] {
        let base_ptr = self.bodies.as_mut_ptr();
        let mut ptrs = ids.map(|id| {
            id.and_then(
                |id| match self.bodies.binary_search_by_key(&id, |&(id, _)| id) {
                    Ok(index) => unsafe {
                        Some(NonNull::new_unchecked(&raw mut (*base_ptr.add(index)).1))
                    },
                    Err(_) => None,
                },
            )
        });
        for i in 1..ptrs.len() {
            for j in 0..i {
                if ptrs[i] == ptrs[j] {
                    ptrs[i] = None;
                }
            }
        }
        unsafe { ptrs.map(|ptr| ptr.map(|mut ptr| ptr.as_mut())) }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (BodyId, &Body)> {
        self.bodies.iter().map(|(id, body)| (*id, body))
    }

    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (BodyId, &mut Body)> {
        self.bodies.iter_mut().map(|(id, body)| (*id, body))
    }

    pub fn iter_mut_pairs(&mut self, mut f: impl FnMut(BodyId, &mut Body, BodyId, &mut Body)) {
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let [(a_id, a), (b_id, b)] = self.bodies.get_disjoint_mut([i, j]).unwrap();
                f(*a_id, a, *b_id, b)
            }
        }
    }
}

impl Default for BodyList {
    fn default() -> Self {
        Self::new()
    }
}
