use crate::{
    body::{Body, BodyId, BodyList},
    camera::Camera,
    universe::Universe,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use std::{borrow::Cow, collections::BTreeMap};

#[derive(Debug)]
pub struct Save<'a> {
    pub current_state: usize,
    pub step_size: f64,
    pub camera: Camera,
    pub states: Cow<'a, [Universe]>,
}

impl Serialize for Save<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("Save", 4)?;
        s.serialize_field("current_state", &self.current_state)?;
        s.serialize_field("step_size", &self.step_size)?;
        s.serialize_field("camera", &self.camera)?;

        struct BodyListSerialiser<'a> {
            body_list: &'a BodyList,
        }

        impl Serialize for BodyListSerialiser<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.collect_seq(
                    self.body_list
                        .iter()
                        .map(|(id, body)| (id.get_id().get(), body)),
                )
            }
        }

        #[derive(Serialize)]
        #[serde(rename = "Universe")]
        struct UniverseSerializer<'a> {
            index: usize,
            gravity: f64,
            bodies: BodyListSerialiser<'a>,
        }

        struct StatesSerializer<'a> {
            states: &'a [Universe],
        }

        impl Serialize for StatesSerializer<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.collect_seq(self.states.iter().enumerate().filter_map(
                    |(index, universe)| {
                        universe.changed.then_some(UniverseSerializer {
                            index,
                            gravity: universe.gravity,
                            bodies: BodyListSerialiser {
                                body_list: &universe.bodies,
                            },
                        })
                    },
                ))
            }
        }

        assert!(self.states[0].changed);
        s.serialize_field(
            "states",
            &StatesSerializer {
                states: &self.states,
            },
        )?;

        s.end()
    }
}

impl<'de> Deserialize<'de> for Save<'_> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "Universe")]
        struct UniverseImpl {
            index: usize,
            gravity: f64,
            bodies: Vec<(usize, Body)>,
        }

        #[derive(Deserialize)]
        #[serde(rename = "Save")]
        struct SaveImpl {
            current_state: usize,
            step_size: f64,
            camera: Camera,
            states: Vec<UniverseImpl>,
        }

        let SaveImpl {
            current_state,
            step_size,
            camera,
            states,
        } = SaveImpl::deserialize(deserializer)?;
        assert_eq!(states[0].index, 0);

        let mut result_states = vec![];

        let mut id_to_body_id = BTreeMap::<usize, BodyId>::new();
        let mut universes = states.into_iter().peekable();
        while let Some(universe) = universes.next() {
            let mut new_universe = Universe {
                bodies: BodyList::new(),
                gravity: universe.gravity,
                changed: true,
            };
            for (id, body) in universe.bodies {
                new_universe.bodies.insert(
                    *id_to_body_id.entry(id).or_insert_with(BodyId::next_id),
                    body,
                );
            }
            result_states.push(new_universe);

            let step_count = universes
                .peek()
                .map_or(current_state, |universe| universe.index)
                .saturating_sub(universe.index);

            for _ in 0..step_count {
                let mut stepped_universe = result_states.last().unwrap().clone();
                stepped_universe.step(step_size);
                result_states.push(stepped_universe);
            }
        }

        Ok(Save {
            current_state,
            step_size,
            camera,
            states: result_states.into(),
        })
    }
}
