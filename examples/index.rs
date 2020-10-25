use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    name: String,
    age: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let users = diskomap::BTree::open_or_create("db/index_db.txt")?;

    let user_name_index = users.create_btree_index(|user: &User|
        user.name.clone()
    )?;

    users.insert(0, User { name: "Masha".to_string(), age: 23 })?;
    users.insert(3, User { name: "Sasha".to_string(), age: 31 })?;
    users.insert(5, User { name: "Pasha".to_string(), age: 33 })?;
    users.insert(17, User { name: "Natasha".to_string(), age: 19 })?;
    users.insert(12, User { name: "Natasha".to_string(), age: 33 })?;

    println!("Users with Natasha name:");
    for user_id in user_name_index.get(&"Natasha".to_string())? {
        let user = users.get(&user_id)?;
        println!("{:?}", &user);
    }

    let age_index = users.create_btree_index(|user: &User|
        user.age - user.age % 30
    )?;

    println!("Users 30 - 40 ages:");
    for user_id in age_index.get(&30)? {
        let user = users.get(&user_id).unwrap();
        println!("{:?}", &user);
    }

    // Multi-threaded index creating:
    //
    // If data is big, creating indexes may take some time.
    // If indexes more then one, then to speed up,
    // you can create them in parallel in several threads.
    let cloned_map = users.clone();
    let t1 = std::thread::spawn(move ||
        cloned_map.create_btree_index(|user: &User|
            user.name.clone()
        )
    );

    let cloned_map = users.clone();
    let t2 = std::thread::spawn(move ||
        cloned_map.create_btree_index(|user: &User|
            user.age - user.age % 30
        )
    );

    let _name_index = t1.join().unwrap();
    let _age_index = t2.join().unwrap();

    Ok(())
}
