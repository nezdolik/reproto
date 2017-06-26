use std::collections::HashMap;
use super::*;

pub struct Variables<'a> {
    variables: HashMap<String, &'a RpType>,
}

impl<'a> Variables<'a> {
    pub fn new() -> Variables<'a> {
        Variables { variables: HashMap::new() }
    }

    pub fn get(&self, key: &String) -> Option<&'a RpType> {
        self.variables.get(key).map(|t| *t)
    }

    pub fn insert(&mut self, key: String, value: &'a RpType) {
        self.variables.insert(key, value);
    }
}