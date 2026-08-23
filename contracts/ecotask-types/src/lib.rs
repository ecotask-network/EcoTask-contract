#![no_std]
//! Shared contract types for the EcoTask contracts.
//!
//! `Task` and `TaskStatus` are the wire format used by the task-registry and
//! read by the reward-engine. They live in their own crate — which contains
//! no `#[contractimpl]` blocks and therefore emits no WASM exports — so both
//! contracts can link against them without the duplicate-symbol collisions
//! that occur when a contract links another contract's crate directly.
//!
//! Soroban `#[contracttype]` values are encoded by variant/field order, so
//! any change here changes the on-chain ABI: keep additions append-only and
//! never reorder fields or variants.

use soroban_sdk::{contracttype, Address, BytesN, String};

/// Lifecycle status of a task in the registry.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum TaskStatus {
    Active,
    Completed,
    Expired,
    Cancelled,
}

/// A task as stored in the task-registry and returned by `get_task`.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Task {
    pub id: u64,
    pub creator: Address,
    pub task_type: String,
    pub location_hash: BytesN<32>,
    pub reward_amount: i128,
    pub max_completions: u32,
    pub completions: u32,
    pub status: TaskStatus,
    pub created_at: u64,
    pub expires_at: u64,
}
