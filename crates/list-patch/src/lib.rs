//! Crate which can plan a sequence of operations to get from one sequence to another.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op<T> {
    Reset(Vec<T>),
    Remove {
        row: usize,
        count: usize,
    },
    Move {
        row: usize,
        count: usize,
        destination: usize,
    },
    Insert {
        row: usize,
        rows: Vec<T>,
    },
    Update {
        row: usize,
        rows: Vec<T>,
    },
}

pub fn plan<T, K>(old: &[T], new: Vec<T>, key: impl Fn(&T) -> K) -> Vec<Op<T>>
where
    T: PartialEq,
    K: Hash + Eq,
{
    let Some(old_index) = index_by_key(old, &key) else {
        return vec![Op::Reset(new)];
    };
    let Some(new_index) = index_by_key(&new, &key) else {
        return vec![Op::Reset(new)];
    };

    let mut ops = Vec::new();

    let kept: Vec<bool> = old
        .iter()
        .map(|row| new_index.contains_key(&key(row)))
        .collect();
    let mut removals = runs(kept.iter().map(|kept| !kept)).collect::<Vec<_>>();
    removals.reverse();
    ops.extend(
        removals
            .into_iter()
            .map(|(row, count)| Op::Remove { row, count }),
    );

    let mut survivor_pos = vec![usize::MAX; old.len()];
    for (pos, old_idx) in (0..old.len()).filter(|&i| kept[i]).enumerate() {
        survivor_pos[old_idx] = pos;
    }

    let matched: Vec<Option<usize>> = new
        .iter()
        .map(|row| old_index.get(&key(row)).copied())
        .collect();

    let target: Vec<usize> = matched.iter().flatten().map(|&i| survivor_pos[i]).collect();
    ops.extend(plan_moves(&target));

    let mut inserts: Vec<Op<T>> = Vec::new();
    let mut updates: Vec<Op<T>> = Vec::new();
    for (row, (item, matched)) in new.into_iter().zip(&matched).enumerate() {
        match *matched {
            None => push_run(
                &mut inserts,
                row,
                item,
                |op| match op {
                    Op::Insert { row, rows } => Some((*row, rows)),
                    _ => None,
                },
                |row, rows| Op::Insert { row, rows },
            ),
            Some(old_idx) if old[old_idx] != item => push_run(
                &mut updates,
                row,
                item,
                |op| match op {
                    Op::Update { row, rows } => Some((*row, rows)),
                    _ => None,
                },
                |row, rows| Op::Update { row, rows },
            ),
            Some(_) => {}
        }
    }
    ops.extend(inserts);
    ops.extend(updates);

    ops
}

fn index_by_key<T, K: Hash + Eq>(rows: &[T], key: impl Fn(&T) -> K) -> Option<HashMap<K, usize>> {
    let mut index = HashMap::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        match index.entry(key(row)) {
            Entry::Occupied(_) => return None,
            Entry::Vacant(slot) => {
                slot.insert(i);
            }
        }
    }
    Some(index)
}

fn runs(flags: impl Iterator<Item = bool>) -> impl Iterator<Item = (usize, usize)> {
    let mut flags = flags.enumerate().peekable();
    std::iter::from_fn(move || {
        let (start, _) = flags.find(|&(_, flag)| flag)?;
        let mut len = 1;
        while flags.next_if(|&(_, flag)| flag).is_some() {
            len += 1;
        }
        Some((start, len))
    })
}

fn push_run<T>(
    ops: &mut Vec<Op<T>>,
    row: usize,
    item: T,
    rows_of: impl Fn(&mut Op<T>) -> Option<(usize, &mut Vec<T>)>,
    make: impl Fn(usize, Vec<T>) -> Op<T>,
) {
    if let Some((start, rows)) = ops.last_mut().and_then(rows_of)
        && start + rows.len() == row
    {
        rows.push(item);
        return;
    }
    ops.push(make(row, vec![item]));
}

fn plan_moves<T>(target: &[usize]) -> Vec<Op<T>> {
    let stays = longest_increasing_subsequence(target);
    let mut working: Vec<usize> = (0..target.len()).collect();
    let mut ops = Vec::new();

    let mut i = 0;
    while i < target.len() {
        if stays[i] {
            i += 1;
            continue;
        }

        let row = position(&working, target[i]);
        let mut count = 1;
        while i + count < target.len()
            && !stays[i + count]
            && working.get(row + count) == Some(&target[i + count])
        {
            count += 1;
        }

        let destination = match i {
            0 => 0,
            _ => position(&working, target[i - 1]) + 1,
        };

        if !(row..=row + count).contains(&destination) {
            ops.push(Op::Move {
                row,
                count,
                destination,
            });
            let block: Vec<usize> = working.drain(row..row + count).collect();
            let at = if destination > row {
                destination - count
            } else {
                destination
            };
            working.splice(at..at, block);
        }

        i += count;
    }

    debug_assert_eq!(working, target);
    ops
}

fn position(working: &[usize], value: usize) -> usize {
    working
        .iter()
        .position(|&v| v == value)
        .expect("every target row is in the working list")
}

fn longest_increasing_subsequence(seq: &[usize]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; seq.len()];

    for (i, &value) in seq.iter().enumerate() {
        let k = tails.partition_point(|&t| seq[t] < value);
        prev[i] = k.checked_sub(1).map(|k| tails[k]);
        if k == tails.len() {
            tails.push(i);
        } else {
            tails[k] = i;
        }
    }

    let mut marks = vec![false; seq.len()];
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        marks[i] = true;
        cursor = prev[i];
    }
    marks
}
