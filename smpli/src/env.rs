use std::collections::HashMap;

use super::value::{ Value, ReferableValue };

#[derive(Debug)]
pub struct Env {
    env: HashMap<String, ReferableValue>,
    tmp_store: HashMap<String, ReferableValue>,
}

impl Env {
    pub fn new() -> Env {
        Env {
            env: HashMap::new(),
            tmp_store: HashMap::new(),
        }
    }

    pub fn fork(&self) -> Env {
        let env = self.env
            .iter()
            .map(|(key, referable)| {
                (key.clone(), referable.hard_clone())
            }).collect();

        let tmp_store = self.tmp_store
            .iter()
            .map(|(key, referable)| {
                (key.clone(), referable.hard_clone())
            }).collect();

        Env {
            env: env,
            tmp_store: tmp_store,
        }
    }

    pub fn map_value(&mut self, name: String, value: Value) -> Option<Value> {
        self.env
            .insert(name, ReferableValue::new(value))
            .map(|rv| rv.clone_value())
    }

    pub fn map_tmp(&mut self, name: String, value: Value) -> Option<Value> {
        self.tmp_store
            .insert(name, ReferableValue::new(value))
            .map(|rv| rv.clone_value())
    }

    pub fn get_value(&self, name: &str) -> Option<Value> {
        self.env.get(name).map(|r| r.clone_value())
    }

    pub fn ref_value(&self, name: &str) -> Option<ReferableValue> {
        self.env.get(name).map(|r| r.ref_clone())
    }

    pub fn get_tmp(&self, name: &str) -> Option<Value> {
        self.tmp_store.get(name).map(|r| r.clone_value())
    }

    pub fn ref_tmp(&self, name: &str) -> Option<ReferableValue> {
        self.tmp_store.get(name).map(|r| r.ref_clone())
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.get_value(name).or(self.get_tmp(name))
    }

    pub fn get_ref(&self, name: &str) -> Option<ReferableValue> {
        self.ref_value(name).or(self.ref_tmp(name))
    }

    pub fn wipe_tmps(&mut self) {
        self.tmp_store.clear();
    }
}
