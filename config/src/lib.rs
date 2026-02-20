
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
    children : Vec<Rc<ConfigurationElement>>,
    parent : Weak<ConfigurationElement>,
    value : mlua::Value,
}

impl ConfigurationElement {
    pub fn from_file(lua: Lua, filename:&str) -> Option<Rc<ConfigurationElement>> {
        Some(Rc::new(ConfigurationElement { children:Vec::new(), parent: Weak::new(), value:mlua::Value::Nil}))
    }

    pub fn find_element(&self, path: &str) -> Option<Rc<ConfigurationElement>> {
        None
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
    #[case("data/tests/ConfigurationElement/Empty.lua", "$.foo", false, mlua::Value::Nil)]
    fn test_create_from_file(#[case] filename:&str, #[case] path:&str, #[case] exists : bool,  #[case] value:mlua::Value) {
        let lua = Lua::new();
        let sut = ConfigurationElement::from_file(lua, filename);
        assert!(sut.is_some());
        let actual = sut.unwrap().as_ref().find_element(path);
        assert_eq!(exists, actual.is_some());
        if actual.is_some() {
            assert_eq!(value, *actual.unwrap().get_value());
        }
    }
}
