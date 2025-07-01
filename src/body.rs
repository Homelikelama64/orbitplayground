use cgmath::*;
use serde::{Deserialize, Serialize, de::Visitor};
use std::{f64::consts::PI, num::NonZeroUsize};

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

impl Serialize for BodyList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter().map(|(_, body)| body))
    }
}

impl<'de> Deserialize<'de> for BodyList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BodyVisitor;

        impl<'de> Visitor<'de> for BodyVisitor {
            type Value = BodyList;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("sequence of Body")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut body_list = BodyList::new();
                while let Some(body) = seq.next_element()? {
                    body_list.push(body);
                }
                Ok(body_list)
            }
        }

        deserializer.deserialize_seq(BodyVisitor)
    }
}
