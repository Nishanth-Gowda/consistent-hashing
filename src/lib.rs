//! Consistent hashing, built step by step.
//!
//! The goal: given a key like `"user123"` and N shards, decide which shard
//! stores it, the way Blobasaur picks a SQLite file on a node.
//!
//! Roadmap:
//! 1. `hash`        – turn a key into a position on a 32-bit ring   (this step)
//! 2. `modulo`      – baseline router: `hash % N`
//! 3. `ring`        – classic consistent hashing, one point per shard
//! 4. `vnodes`      – ring with many virtual nodes per shard
//! 5. `multiprobe`  – multi-probe consistent hashing (Blobasaur's choice)
//! 6. a simulation binary that compares all four

pub mod hash;
