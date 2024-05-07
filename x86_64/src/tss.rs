//! This module implements functionality for the x86_64 task state segment (TSS)
//!
//! - In long mode the TSS not store information on a task's execution state,
//! instead it is used to store the Interrupt Stack Table.
//! - The interrupt stack table contains 7 pointers to known "good" stacks
//! - This is needed to e.g. avoid the cpu pushing the exception frame onto
//! the stack when access to the stack causes page faults, which would then cause
//! a triple fault
//!
//!
//!
//!

pub struct TaskStateSegment {
    stack_pointers: [Option<u64>; 7],
}
