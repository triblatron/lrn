use std::cell::RefCell;
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

    pub fn new(name:String, value:mlua::Value) -> Rc<RefCell<ConfigurationElement>> {
        let this = ConfigurationElement{ name: name, value: value, parent:Weak::new(), children:Vec::new()};
        Rc::new(RefCell::new(this))
    }
    pub fn build_tree(lua: &Lua) -> Option<Rc<RefCell<ConfigurationElement>>> {
        let table:Result<mlua::Table,LuaError>  = lua.globals().get("root");
        let mut parent_stack:Vec<Rc<RefCell<ConfigurationElement>>> = vec![];
        let parent = Rc::new(RefCell::new(ConfigurationElement{ name: String::from("root"), children: Vec::new(), parent : Weak::new(), value: mlua::Value::Nil }));
        parent_stack.push(parent.clone());
        let level:u32 = 0;
        if let Ok(table) = table {
            ConfigurationElement::build_tree_helper(lua, table, &mut parent_stack, level);
        }
        // Traverse the table.
        return Some(parent);
    }

    fn build_tree_element(lua: &Lua, name:String, value:mlua::Value, parent_stack:&mut Vec<Rc<RefCell<ConfigurationElement>>>, level:u32) -> () {
        if value.is_string() || value.is_integer() || value.is_number() || value.is_boolean() {
            let element =  ConfigurationElement::new(name, value.clone());

            let top = parent_stack.last().unwrap();
            top.borrow_mut().add_child(top, element);
        }
        else if value.is_table() {
            let child = ConfigurationElement::new(name, mlua::Value::Nil);

            let top  = parent_stack.last().unwrap();
            top.borrow_mut().add_child(top, child.clone());
            parent_stack.push(child.clone());
            Self::build_tree_helper(&lua, value.as_table().unwrap().clone(), parent_stack, level+1);
        }
    }
    pub fn build_tree_helper(lua: &Lua, table: mlua::Table, parent_stack: &mut Vec<Rc<RefCell<ConfigurationElement>>>, level:u32) {
        let table:mlua::Table = table;
        for pair in table.pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = pair.unwrap();
            while parent_stack.len() - 1 > level as usize {
                parent_stack.pop();
            }
            if key.is_string() {
                Self::build_tree_element(lua, key.to_string().unwrap(), value, parent_stack, level);
            }
            else if key.is_integer() {
                let mut name:String = String::from("[");
                name.push_str(key.to_string().unwrap().as_str());
                name.push_str("]");
                Self::build_tree_element(lua, name, value, parent_stack, level);
            }
        }
    }

    pub fn find_element(&self, path: &str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if path.starts_with("$") {
            let relative_path = path.strip_prefix("$").unwrap();

            let self_rc = Rc::new(RefCell::new(self.clone()));
            let mut root = Rc::downgrade(&self_rc);
            let mut parent = root.clone();
            while let Some(some_parent) = parent.upgrade() {
                root = Rc::downgrade(&some_parent);
                parent = some_parent.borrow().parent.clone();
            }

            let dot_pos = relative_path.find('.');

            if let Some(pos) = dot_pos {

                return root.upgrade().unwrap().borrow().find_element(&relative_path[pos+1..]);
            }
            else {
                return Some(root.upgrade().unwrap());
            }
        }

        return self.find_in_children(path);
    }

    pub fn find_in_children(&self, path: &str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if self.name == path {
            return Some(Rc::new(RefCell::new(self.clone())));
        }
        let dot_pos = path.find('.');
        if let Some(dot_pos) = dot_pos {
            let name = &path[0..dot_pos];
            for child in &self.children {
                if name == child.borrow().name {
                    let candidate = child.borrow().find_in_children(&path[dot_pos+1..]);
                    if let Some(candidate) = candidate {
                        return Some(candidate.clone());
                    }
                }
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

    pub fn add_child(&mut self, self_rc:&Rc<RefCell<ConfigurationElement>>, child:Rc<RefCell<ConfigurationElement>>) {
        child.deref().borrow_mut().parent = Rc::downgrade(&self_rc);
        self.children.push(child.clone());
    }
    pub fn get_value(&self) -> &mlua::Value {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn assert_comparison(value: VariantType, actual:&mlua::Value) {
        match value {
            VariantType::Nil => { assert!(actual.is_nil()); }
            VariantType::Boolean(value) => {
                assert!(actual.is_boolean());
                assert_eq!(value, actual.as_boolean().unwrap())
            }
            VariantType::Integer(value) => {
                assert!(actual.is_integer());
                assert_eq!(value, actual.as_integer().unwrap())
            },
            VariantType::Float(value) => {
                assert!(actual.is_number());
                assert_eq!(value, actual.as_number().unwrap())
            },
            VariantType::String(value) => {
                assert!(actual.is_string());
                assert_eq!(value, *actual.as_string().unwrap().to_str().unwrap())
            },
        }
    }
    #[rstest]
    #[case("data/tests/ConfigurationElement/Empty.lua", "foo", false, "", VariantType::Nil)]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "foo", true, "foo", VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "bar", false, "bar", VariantType::Nil)]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "$.foo", true, "foo", VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/NestedElement.lua", "foo.bar", true, "bar", VariantType::Float(1.0))]
    #[case("data/tests/ConfigurationElement/NestedElement.lua", "$.foo.bar", true, "bar", VariantType::Float(1.0))]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "baz", true, "baz", VariantType::String(String::from("wibble")))]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "qux", true, "qux", VariantType::Integer(1))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[1]", true, "[1]", VariantType::Boolean(true))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[2]", true, "[2]", VariantType::Float(2.0))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[3]", true, "[3]", VariantType::String(String::from("wibble")))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[4].bar", true, "bar", VariantType::Float(1.5))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "$.[1]", true, "[1]", VariantType::Integer(2))]
    fn test_create_from_file(#[case] filename:&str, #[case] path:&str, #[case] exists : bool,  #[case] name: &str, #[case] value:VariantType) {
        let lua = Lua::new();
        let sut = ConfigurationElement::from_file(&lua, filename);
        assert!(sut.is_some());
        let actual = sut.unwrap().as_ref().borrow().find_element(path);
        assert_eq!(exists, actual.is_some());
        if let Some(actual) = actual {
            let actual = actual.deref().borrow();
            assert_eq!(name, actual.name);
            assert_comparison(value, actual.get_value());
            //assert_eq!(value, *actual.unwrap().borrow().get_value());
        }
    }

    #[rstest]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "$", "$.baz", VariantType::String(String::from("wibble")))]
    #[case("data/tests/ConfigurationElement/NestedMultipleChildren.lua", "foo.bar", "$.baz", VariantType::String(String::from("wibble")))]
    #[case("data/tests/ConfigurationElement/IntegerIndex.lua", "foo.[4]", "bar", VariantType::Float(1.5))]
    fn test_find_element(#[case] filename:&str, #[case] path_to_location:&str, #[case] absolute_path:&str, #[case] value:VariantType) {
        let lua = Lua::new();
        let sut = ConfigurationElement::from_file(&lua, filename);
        assert!(sut.is_some());
        let sut = sut.unwrap();
        let location = sut.as_ref().borrow().find_element(path_to_location);
        assert!(location.is_some());
        let location = location.unwrap();
        let actual = location.as_ref().borrow().find_element(absolute_path);
        assert!(actual.is_some());
        assert_comparison(value, actual.unwrap().deref().borrow().get_value());
    }

}
