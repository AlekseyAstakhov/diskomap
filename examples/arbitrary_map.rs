use diskomap::map_trait::MapTrait;
use diskomap::map_with_file::MapWithFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "db/arbitrary_map.txt";

    // VecMapWithFile based on vector_map::VecMap
    let mut map = StupidMapWithFile::open_or_create(file_name, diskomap::Cfg::default())?;
    map.insert(0, "Masha".to_string())?;
    map.insert(1, "Sasha".to_string())?;
    map.insert(3, "Natasha".to_string())?;
    map.remove(&1)?;
    drop(map);

    println!("File content:");
    print!("{}", std::fs::read_to_string(file_name)?);

    Ok(())
}

/// Simple vec map.
struct StupidMap<Key, Value> {
    vec: Vec<(Key, Value)>
}

// File based map from 'StupidMap'.
type StupidMapWithFile<Key, Value> = MapWithFile<Key, Value, StupidMap<Key, Value>>;

// Implementation of the required interface for the file based map wrapper.
impl<Key: Ord, Value>  MapTrait<Key, Value>  for StupidMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> {
        let res = self.vec.binary_search_by(|(k, _)| {
            k.cmp(key)
        });

        if let Ok(index) = res {
            self.vec.get(index).map(|(_, val)| val)
        } else {
            None
        }
    }

    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        let res = self.vec.binary_search_by(|(k, _)| {
            k.cmp(key)
        });

        if let Ok(index) = res {
            self.vec.get_mut(index).map(|(_, val)| val)
        } else {
            None
        }
    }

    fn insert(&mut self, key: Key, value: Value)  -> Option<Value> {
        let res = self.vec.binary_search_by(|(k, _)| {
            k.cmp(&key)
        });

        let mut value = value;

        if let Ok(index) = res {
            if let Some(current_val) = self.vec.get_mut(index).map(|(_, val)| val) {
                std::mem::swap(current_val, &mut value);
                Some(value)
            } else {
                unreachable!();
            }
        } else {
            self.vec.push((key, value));
            self.vec.sort_by(|(key_a, _), (key_b, _)| { key_a.cmp(key_b) });
            None
        }
    }

    fn remove(&mut self, key: &Key) -> Option<Value> {
        let res = self.vec.binary_search_by(|(k, _)| {
            k.cmp(key)
        });

        if let Ok(index) = res {
            let (_, old_val) = self.vec.remove(index);
            Some(old_val)
        } else {
            None
        }
    }

    fn for_each(&self, mut f: impl FnMut(&Key, &Value)) {
        for (key, val) in self.vec.iter() {
            f(key, val)
        }
    }
}

impl<Key: Default, Value: Default> Default for StupidMap<Key, Value> {
    fn default() -> Self {
        StupidMap { vec: Vec::new() }
    }
}
