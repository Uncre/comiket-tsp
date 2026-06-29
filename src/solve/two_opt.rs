//! 2-opt and Or-opt local search on a cyclic tour with a frozen leading prefix.
//!
//! Both moves recompute only the handful of edges they touch (incremental cost), and
//! the search applies improving moves until neither neighbourhood offers one — a
//! 2-opt/Or-opt local optimum. Positions `0..lock` never move (see [`super`]).

use super::{Cost, EPS};

/// Drive `tour` to a 2-opt + Or-opt local optimum in place.
pub(crate) fn local_search(cost: &Cost, tour: &mut [usize], lock: usize) {
    loop {
        let mut improved = false;
        improved |= two_opt_pass(cost, tour, lock);
        improved |= or_opt_pass(cost, tour, lock);
        if !improved {
            break;
        }
    }
}

/// One sweep of 2-opt segment reversals, applying every improving move it finds.
fn two_opt_pass(cost: &Cost, tour: &mut [usize], lock: usize) -> bool {
    let len = tour.len();
    if len < 4 {
        return false;
    }
    let mut improved = false;
    for i in lock..(len - 1) {
        for j in (i + 1)..len {
            let a = tour[i - 1];
            let b = tour[i];
            let c = tour[j];
            let d = tour[(j + 1) % len];
            // Reversing tour[i..=j] swaps edges (a,b)+(c,d) for (a,c)+(b,d).
            let delta = cost.d(a, c) + cost.d(b, d) - cost.d(a, b) - cost.d(c, d);
            if delta < -EPS {
                tour[i..=j].reverse();
                improved = true;
            }
        }
    }
    improved
}

/// One sweep of Or-opt relocations (segment length 1–3, either orientation),
/// applying the first improving move found.
fn or_opt_pass(cost: &Cost, tour: &mut [usize], lock: usize) -> bool {
    let len = tour.len();
    for seg_len in 1..=3usize {
        if lock + seg_len >= len {
            break; // no room: the segment would be the whole movable region
        }
        for s in lock..=(len - seg_len) {
            let prev = tour[s - 1];
            let next = tour[(s + seg_len) % len];
            let seg_first = tour[s];
            let seg_last = tour[s + seg_len - 1];
            // Cost recovered by lifting the segment out and closing the gap.
            let remove_gain = cost.d(prev, seg_first) + cost.d(seg_last, next) - cost.d(prev, next);

            for p in 0..len {
                if p + 1 < lock {
                    continue; // edge lies inside the locked prefix
                }
                if (s - 1..s + seg_len).contains(&p) {
                    continue; // edge is incident to / inside the segment
                }
                let u = tour[p];
                let v = tour[(p + 1) % len];
                let base = cost.d(u, v);
                let fwd = cost.d(u, seg_first) + cost.d(seg_last, v) - base - remove_gain;
                let rev = cost.d(u, seg_last) + cost.d(seg_first, v) - base - remove_gain;
                let (delta, reversed) = if fwd <= rev {
                    (fwd, false)
                } else {
                    (rev, true)
                };
                if delta < -EPS {
                    relocate(tour, s, seg_len, p, reversed);
                    return true;
                }
            }
        }
    }
    false
}

/// Move `tour[s..s+seg_len]` to sit immediately after position `p`, optionally
/// reversed, preserving every other node's relative order.
fn relocate(tour: &mut [usize], s: usize, seg_len: usize, p: usize, reversed: bool) {
    let len = tour.len();
    let seg: Vec<usize> = tour[s..s + seg_len].to_vec();
    let mut rebuilt = Vec::with_capacity(len);
    for (pos, &node) in tour.iter().enumerate() {
        if pos >= s && pos < s + seg_len {
            continue; // skip the lifted segment
        }
        rebuilt.push(node);
        if pos == p {
            if reversed {
                rebuilt.extend(seg.iter().rev());
            } else {
                rebuilt.extend(seg.iter());
            }
        }
    }
    tour.copy_from_slice(&rebuilt);
}
