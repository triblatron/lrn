use std::cell::RefCell;
use std::fs;
use std::fs::exists;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;
use rstest;
use mlua::prelude::*;

enum VariantType {
    Integer(i64),
    Float(f64),
    String(String),
}

struct Variant {
    value : Option<VariantType>
}
struct ConfigurationElement {
    name: String,
    children : Vec<Rc<RefCell<ConfigurationElement>>>,
    parent : Weak<RefCell<ConfigurationElement>>,
    value : mlua::Value,
}

impl ConfigurationElement {
    pub fn from_file(lua: Lua, filename:&str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        if let Ok(_) = exists(filename) {
            let code = fs::read_to_string(filename);
            if let Ok(code) = code {
                let chunk = lua.load(code);
                let result = chunk.exec();
                match (result) {
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

    pub fn build_tree(lua: Lua) -> Option<Rc<RefCell<ConfigurationElement>>> {
        let table:Result<mlua::Table,LuaError>  = lua.globals().get("root");
        let mut parent = Rc::new(RefCell::new(ConfigurationElement{ name: String::from("root"), children: Vec::new(), parent : Weak::new(), value: mlua::Value::Nil }));
        if let Ok(table) = table {
            let table:mlua::Table = table;
            for pair in table.pairs::<mlua::Value, mlua::Value>() {
                let (key, value) = pair.unwrap();
                println!("{:?} = {:?}", key, value);
                if key.is_string() {
                    let element = Rc::new(RefCell::new(ConfigurationElement { name:key.to_string().unwrap(), children : Vec::new(), parent : Weak::new(), value: value }));
                    parent.borrow_mut().add_child(element);
                }
            }

        }
        // Traverse the table.
        return Some(parent);
    }

    pub fn find_element(&self, path: &str) -> Option<Rc<RefCell<ConfigurationElement>>> {
        for child in &self.children {
            if child.borrow().name == path {
                return Some(child.clone());
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
    use std::ops::Deref;
    use super::*;
    use rstest::rstest;
    #[rstest]
    #[case("data/tests/ConfigurationElement/Empty.lua", "foo", false, mlua::Value::Nil)]
    #[case("data/tests/ConfigurationElement/OneElement.lua", "foo", true, mlua::Value::Boolean(true))]
    fn test_create_from_file(#[case] filename:&str, #[case] path:&str, #[case] exists : bool,  #[case] value:mlua::Value) {
        let lua = Lua::new();
        let sut = ConfigurationElement::from_file(lua, filename);
        assert!(sut.is_some());
        let actual = sut.unwrap().as_ref().borrow().find_element(path);
        assert_eq!(exists, actual.is_some());
        if actual.is_some() {
            assert_eq!(value, *actual.unwrap().borrow().get_value());
        }
    }
}
