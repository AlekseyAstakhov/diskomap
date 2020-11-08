use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    name: String,
    age: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut users = diskomap::BTree::open_or_create("db/index_db.txt", None)?;

    // Indexes can be created to quickly retrieve elements by value content,
    // they can be as btree or as hashmap.

    // The btree index can be of a any type that implements Ord and Clone trait.
    // Usually index is part of the value. Use of data not included in the value should be avoided.
    // For example, the index for getting users by name looks like this:
    let user_name_index = users.create_btree_index(|user: &User|
        // This closure will be called every time when insert or delete to the users btree.
        user.name.clone()
    );

    users.insert(0, User { name: "Masha".to_string(), age: 23 })?;
    users.insert(3, User { name: "Sasha".to_string(), age: 31 })?;
    users.insert(5, User { name: "Pasha".to_string(), age: 33 })?;
    users.insert(17, User { name: "Natasha".to_string(), age: 19 })?;
    users.insert(12, User { name: "Natasha".to_string(), age: 33 })?;

    println!("Users with Natasha name:");
    for user_id in user_name_index.get(&"Natasha".to_string()) {
        let user = users.get(&user_id);
        println!("{:?}", &user);
    }

    // Also the index can be more complex than part of the value. For example:
    let age_index = users.create_btree_index(|user: &User|
        user.age - user.age % 30
    );

    println!("Users 30 - 40 ages:");
    for user_id in age_index.get(&30) {
        let user = users.get(&user_id);
        println!("{:?}", &user);
    }

    Ok(())
}
