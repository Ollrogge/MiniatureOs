extern crate alloc;
use alloc::collections::BTreeMap;
use core::ops::Range;

pub struct RangeAllocator {
    total_range: Range<u64>,
    allocated_ranges: BTreeMap<u64, Range<u64>>,
}

impl RangeAllocator {
    pub const fn new(total_range: Range<u64>) -> Self {
        Self {
            total_range,
            allocated_ranges: BTreeMap::new(),
        }
    }

    pub fn try_allocate_size(&mut self, size: usize) -> Option<Range<u64>> {
        let mut candidate_range: Option<Range<u64>> = None;
        let mut hole_size: u64 = u64::MAX;
        let mut last_end = self.total_range.start;
        // search for an appropriate hole in the allocated ranges using a best-fit
        // approach
        for allocated in self.allocated_ranges.values() {
            if allocated.start > last_end {
                let potential_range = Range {
                    start: last_end,
                    end: last_end + size as u64,
                };

                if potential_range.end <= allocated.start && self.contains_range(&potential_range) {
                    let tmp_hole_size = allocated.start - potential_range.end;

                    if tmp_hole_size < hole_size {
                        candidate_range = Some(potential_range);
                        hole_size = tmp_hole_size;
                    }
                }
            }
            last_end = allocated.end;
        }

        if candidate_range.is_none() {
            let potential_range = Range {
                start: last_end,
                end: last_end + size as u64,
            };

            if self.contains_range(&potential_range) {
                candidate_range = Some(potential_range)
            }
        }

        if let Some(ref range) = candidate_range {
            self.allocated_ranges.insert(range.start, range.clone());
        }

        candidate_range
    }

    pub fn contains_range(&self, range: &Range<u64>) -> bool {
        self.total_range.start <= range.start && self.total_range.end >= range.end
    }

    pub fn ranges_overlap(r1: &Range<u64>, r2: &Range<u64>) -> bool {
        r1.start < r2.end && r1.end > r2.start
    }

    pub fn try_allocate_range(&mut self, range: Range<u64>) -> bool {
        let not_contained: bool = self
            .allocated_ranges
            .values()
            .all(|allocated_range| !Self::ranges_overlap(allocated_range, &range));

        if not_contained && self.contains_range(&range) {
            self.allocated_ranges.insert(range.start, range);
            true
        } else {
            false
        }
    }

    pub fn deallocate_range(&mut self, range: Range<u64>) -> bool {
        if let Some(stored_range) = self.allocated_ranges.get(&range.start) {
            if *stored_range == range {
                self.allocated_ranges.remove(&range.start);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_range_allocator_size() {
        let intial_range = Range {
            start: 0,
            end: 1000,
        };
        let mut allocator = RangeAllocator::new(intial_range);

        let a0 = allocator.try_allocate_size(1000).unwrap();
        assert_eq!(
            a0,
            Range {
                start: 0,
                end: 1000
            }
        );

        assert_eq!(allocator.deallocate_range(a0), true);

        let a1 = allocator.try_allocate_size(200).unwrap();
        assert_eq!(a1, Range { start: 0, end: 200 });
        let a2 = allocator.try_allocate_size(200).unwrap();
        assert_eq!(
            a2,
            Range {
                start: 200,
                end: 400
            }
        );
        let a3 = allocator.try_allocate_size(200).unwrap();
        assert_eq!(
            a3,
            Range {
                start: 400,
                end: 600
            }
        );
        let a4 = allocator.try_allocate_size(100).unwrap();
        assert_eq!(
            a4,
            Range {
                start: 600,
                end: 700
            }
        );
        let a5 = allocator.try_allocate_size(200).unwrap();
        assert_eq!(
            a5,
            Range {
                start: 700,
                end: 900
            }
        );

        assert_eq!(allocator.deallocate_range(a2), true);
        assert_eq!(allocator.deallocate_range(a4), true);

        let a4_new = allocator.try_allocate_size(100).unwrap();
        assert_eq!(
            a4_new,
            Range {
                start: 600,
                end: 700
            }
        );
        let a2_new = allocator.try_allocate_size(200).unwrap();
        assert_eq!(
            a2_new,
            Range {
                start: 200,
                end: 400
            }
        );

        let a6 = allocator.try_allocate_size(100).unwrap();
        assert_eq!(
            a6,
            Range {
                start: 900,
                end: 1000
            }
        );

        let a7 = allocator.try_allocate_size(1);
        assert_eq!(a7, None);
    }

    #[test]
    fn test_range_allocator_range() {
        let intial_range = Range {
            start: 0,
            end: 1000,
        };
        let mut allocator = RangeAllocator::new(intial_range);

        let r1 = Range {
            start: 0,
            end: 1000,
        };
        assert_eq!(allocator.try_allocate_range(r1.clone()), true);

        assert_eq!(allocator.deallocate_range(r1), true);

        let r2 = Range { start: 0, end: 400 };
        assert_eq!(allocator.try_allocate_range(r2.clone()), true);

        let r3 = Range {
            start: 700,
            end: 1000,
        };
        assert_eq!(allocator.try_allocate_range(r3.clone()), true);

        let r4 = Range {
            start: 500,
            end: 701,
        };
        assert_eq!(allocator.try_allocate_range(r4.clone()), false);

        let r5 = Range {
            start: 500,
            end: 600,
        };
        assert_eq!(allocator.try_allocate_range(r5.clone()), true);

        assert_eq!(allocator.deallocate_range(r5), true);

        let r6 = Range {
            start: 400,
            end: 700,
        };
        assert_eq!(allocator.try_allocate_range(r6.clone()), true);
    }
}
