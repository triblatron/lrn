use std::cell::RefCell;
use std::fmt::Display;
use std::fs;
use std::fs::exists;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;
use rstest;
use mlua::prelude::*;

enum VariantType {
    Nil,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}

struct Variant {
    value : Option<VariantType>
}

#[derive(Clone)]
struct ConfigurationElement {
    name: String,
    children : Vec<Rc<RefCell<ConfigurationElement>>>,
    parent : Weak<RefCell<ConfigurationElement>>,
    value : mlua::Value,
}

impl ConfigurationElement {
    pub fn from_file(lua: &Lua, filename:&str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if let Ok(_) = exists(filename) {
            let code = fs::read_to_string(filename);
            if let Ok(code) = code {
                let chunk = lua.load(code);
                let result = chunk.exec();
                match result {
                    Ok(()) => {
                        return ConfigurationElement::build_tree(lua);
                    }
                    Err(e) => {
                        eprintln!("Error loading configuration element: {}", e);

                    }
                }
            }
        }
        None
    }

    pub fn build_tree(lua: &Lua) -> Option<Rc<RefCell<ConfigurationElement>>> {
        let table:Result<mlua::Table,LuaError>  = lua.globals().get("root");
        let mut parent_stack:Vec<Rc<RefCell<ConfigurationElement>>> = vec![];
        let mut parent = Rc::new(RefCell::new(ConfigurationElement{ name: String::from("root"), children: Vec::new(), parent : Weak::new(), value: mlua::Value::Nil }));
        parent_stack.push(parent.clone());
        let mut level:u32 = 0;
        if let Ok(table) = table {
            ConfigurationElement::build_tree_helper(lua, table, &mut parent_stack, level);
        }
        // Traverse the table.
        return Some(parent);
    }

    pub fn build_tree_helper(lua: &Lua, table: mlua::Table, parent_stack: &mut Vec<Rc<RefCell<ConfigurationElement>>>, level:u32) {
        let table:mlua::Table = table;
        for pair in table.pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = pair.unwrap();
            println!("{:?} = {:?}", key, value);
            while parent_stack.len() - 1 > level as usize {
                parent_stack.pop();
            }
            if key.is_string() {
                if value.is_string() || value.is_integer() || value.is_number() || value.is_boolean() {
                    let element = Rc::new(RefCell::new(ConfigurationElement { name:key.to_string().unwrap(), children : Vec::new(), parent : Weak::new(), value: value.clone() }));

                    parent_stack.last_mut().unwrap().borrow_mut().add_child(element);
                }
                else if value.is_table() {
                    let child = Rc::new(RefCell::new(ConfigurationElement { name:key.to_string().unwrap(), children:Vec::new(), parent:Weak::new(), value: mlua::Value::Nil }));

                    parent_stack.last_mut().unwrap().borrow_mut().add_child(child.clone());
                    parent_stack.push(child.clone());
                    Self::build_tree_helper(&lua, value.as_table().unwrap().clone(), parent_stack, level+1);
                }
            }
            else if key.is_integer() {
                if value.is_string() || value.is_integer() || value.is_number() || value.is_boolean() {
                    let mut name:String = String::from("[");
                    name.push_str(key.to_string().unwrap().as_str());
                    name.push_str("]");
                    let element = Rc::new(RefCell::new(ConfigurationElement { name:name, children : Vec::new(), parent : Weak::new(), value: value.clone() }));

                    parent_stack.last_mut().unwrap().borrow_mut().add_child(element);
                }
                else if value.is_table() {
                    let child = Rc::new(RefCell::new(ConfigurationElement { name:key.to_string().unwrap(), children:Vec::new(), parent:Weak::new(), value: mlua::Value::Nil }));

                    parent_stack.last_mut().unwrap().borrow_mut().add_child(child.clone());
                    parent_stack.push(child.clone());
                    Self::build_tree_helper(&lua, value.as_table().unwrap().clone(), parent_stack, level+1);
                }
            }
        }
    }

    pub fn find_element(&self, path: &str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if path.starts_with("$.") {
            let relative_path = path.strip_prefix("$.").unwrap();

            return self.find_in_children(relative_path);
        }

        return self.find_in_children(path);
    }

    pub fn find_in_children(&self, path: &str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if self.name == path {
            return Some(Rc::new(RefCell::new(self.clone())));
        }
        let dot_pos = path.find('.');
        if let Some(dot_pos) = dot_pos {
            for child in &self.children {
                return child.borrow().find_in_children(&path[dot_pos+1..])
            }
        }
        else {
            for child in &self.children {
                if child.borrow().name == path {
                    return Some(child.clone());
                }
            }
        }
        None
    }

    pub fn add_child(&mut self, child:Rc<RefCell<ConfigurationElement>>) {
        self.children.push(child.clone());
        child.deref().borrow_mut().parent = Rc::downgrade(&child);
    }
    pub fn get_value(&self) -> &mlua::Value {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    #[rstest]
    #[case("data/tests/ConfigurationElement/Empty.lua", "foo", false, VariantType::Nil)]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "foo", true, VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "$.foo", true, VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/NestedElement.lua", "foo.bar", true, VariantType::Float(1.0))]
    #[case("data/tests/ConfigurationElement/NestedElement.lua", "$.foo.bar", true, VariantType::Float(1.0))]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "baz", true, VariantType::String(String::from("wibble")))]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "qux", true, VariantType::Integer(1))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[1]", true, VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[2]", true, VariantType::Float(2.0))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[3]", true, VariantType::String(String::from("wibble")))]
    fn test_create_from_file(#[case] filename:&str, #[case] path:&str, #[case] exists : bool,  #[case] value:VariantType) {
        let lua = Lua::new();
        let sut = ConfigurationElement::from_file(&lua, filename);
        assert!(sut.is_some());
        let actual = sut.unwrap().as_ref().borrow().find_element(path);
        assert_eq!(exists, actual.is_some());
        if let Some(actual) = actual {
            match value {
                VariantType::Nil => { assert!(actual.deref().borrow().get_value().is_nil()); }
                VariantType::Boolean(value) => assert_eq!(value, actual.deref().borrow().get_value().as_boolean().unwrap()),
                VariantType::Integer(value) => assert_eq!(value, actual.deref().borrow().get_value().as_integer().unwrap()),
                VariantType::Float(value) => assert_eq!(value, actual.deref().borrow().get_value().as_number().unwrap()),
                VariantType::String(value) => assert_eq!(value, *actual.deref().borrow().get_value().as_string().unwrap().to_str().unwrap()),
            }
            //assert_eq!(value, *actual.unwrap().borrow().get_value());
        }
    }
}
