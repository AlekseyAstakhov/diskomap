# diskomap
At the moment it contains wrappers around std::collections::BTreeMap and std::collections::HashMap with saving all operations to a history file for restoring the state, say, when the application is restarted. 

Saving to a file is done in a format similar to ndjson with the ability to use data integrity checks using checksums for each row or blockchain hashes.

Easy convertation functional if you wanna change key-values types or change storing options.
It is possible to use nostd map based collections.
Also supports building arbitrary indexes by value content.

This crate can be used for build in memmory databases if all the data fits into RAM or if you need very fast and competitive data access.

### License

Licensed under either of
* Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT) at your option.

at your option.
