use vector_map::VecMap;
use diskomap::map_trait::MapTrait;
use diskomap::map_with_file::MapWithFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "db/alternative_map.txt";

    // VecMapWithFile based on vector_map::VecMap
    let mut map = VecMapWithFile::open_or_create(file_name, diskomap::Cfg::default())?;

    map.insert(0, "Masha".to_string())?;
    map.insert(1, "Sasha".to_string())?;
    map.insert(0, "Natasha".to_string())?;

    println!("File content:");
    print!("{}", std::fs::read_to_string(file_name)?);

    Ok(())
}

// Need because error
// 'type parameter `Key` must be used as the type parameter for some local type'
// if implement directly.
struct VecMapLocal<Key, Value>(VecMap<Key, Value>);

// Map with storing all changes history to the file based on 'vector_map::VecMap'.
type VecMapWithFile<Key, Value> = MapWithFile<Key, Value, VecMapLocal<Key, Value>>;

// Implementation of the required interface for the file based map wrapper.
impl<Key: Eq, Value>  MapTrait<Key, Value>  for VecMapLocal<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.0.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.0.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value)  -> Option<Value> { self.0.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.0.remove(key) }
    fn for_each(&self, mut f: impl FnMut(&Key, &Value)) { for (key, val) in self.0.iter() { f(key, val) } }
}

impl<Key: Default, Value: Default> Default for VecMapLocal<Key, Value> {
    fn default() -> Self {
        VecMapLocal(VecMap::default())
    }
}
