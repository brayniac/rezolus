//! Block device slots for per-device block I/O maps.
//!
//! A block layer hook has the request's disk in hand, whose `dev_t` is a
//! sparse 32-bit key that cannot index a map directly; a slot can. This
//! module decides which disk holds which slot. The reader
//! (`blockdev_reader.rs`) carries out its decisions on the BPF lookup map
//! (`blk_slots` in `bpf/blockdev.h`), the maps and the slot identities.
//!
//! # Which disks get a slot
//!
//! A disk gets a slot when the BPF program reports a request completing on
//! it: a request whose `dev_t` the lookup does not hold is counted in slot 0
//! and its `dev_t` and disk name are written to `blk_unknown`, which the
//! reader drains on each refresh. So only disks that complete requests through
//! the block layer get slots. A bio-based device never does: the I/O of
//! dm-linear, dm-crypt, LVM and md is counted on the disks beneath them, and
//! zram and ram devices, which do not use requests, are not counted. An idle
//! loop device holds no slot.
//!
//! # Slot 0 is "other"
//!
//! Every request lands in some slot, so the per-device sums are complete. Slot
//! 0, labelled `device="other"`, counts requests to a disk before the next
//! refresh gives it a slot, and to every disk once the 32 assignable slots are
//! taken; such a disk is mapped to slot 0 in the lookup so it is not reported
//! again until a slot is released (its disk left `/sys/block`, or the reader
//! reclaimed it), when every disk in slot 0 is unmapped and reports again.
//! The reader releases the slot whose disk has been idle longest, if that is
//! at least [`EVICT_IDLE`], in two cases: a new disk reports while every slot
//! is taken (the slot goes to the new disk), and a rescan finds disks in slot
//! 0 (the slot is freed for them). A loop device detached with `losetup -d`
//! stays in `/sys/block`, so without this it would keep its slot.
//!
//! # NVMe multipath
//!
//! With native NVMe multipath, requests complete on a hidden path disk
//! (`nvme0c0n1`, `nvme<subsystem>c<controller>n<namespace>`) that has no
//! `dev` in sysfs. A path is labelled with its head disk's name
//! (`nvme0n1`), and every path of one namespace maps to the head's slot.
//!
//! # Stability
//!
//! A disk keeps its slot while it is present. Every [`RESCAN_INTERVAL`] the
//! reader lists `/sys/block`; a disk whose name is gone, or whose `dev` there
//! is no longer the one it was mapped by, loses its mapping, and a slot with no
//! disk left is released. Until that listing, a new disk that reuses a mapped
//! `dev_t` is counted under the old disk's label; a `dev_t` that is reported
//! again under another name is a different disk. The reader zeroes a released
//! slot before another disk takes it.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;

use crate::agent::MAX_BLOCK_DEVICES;

/// How often the reader lists `/sys/block` to free the slots of disks that are
/// gone.
pub const RESCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How long a slot's sums must have been unchanged before the reader evicts it
/// for a new disk when every slot is taken.
pub const EVICT_IDLE: Duration = Duration::from_secs(600);

pub const SYS_BLOCK: &str = "/sys/block";

/// The label slot 0 carries.
pub const OTHER: &str = "other";

/// What the reader does to carry out a decision, in the order given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Remove `dev` from the lookup, and let BPF report it again.
    Unmap { dev: u32 },
    /// Release `slot`: clear and zero its values and its identity. Every
    /// `dev` mapped to it has been unmapped first.
    Release { slot: usize },
    /// Give `slot` to the disk `label`. Its values are zero.
    Assign { slot: usize, label: String },
    /// Map `dev` to `slot` in the lookup (slot 0 for a disk beyond the cap).
    Map { dev: u32, slot: usize },
}

/// A `dev_t` the lookup holds, and the disk name it was reported under.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Alias {
    slot: usize,
    name: String,
}

/// Which disk holds which slot, and which `dev_t`s map to each.
#[derive(Debug)]
pub struct SlotTable {
    /// Index is the slot; slot 0 is never assigned.
    slots: Vec<Option<String>>,
    devs: HashMap<u32, Alias>,
}

impl Default for SlotTable {
    fn default() -> Self {
        Self {
            slots: vec![None; MAX_BLOCK_DEVICES],
            devs: HashMap::new(),
        }
    }
}

impl SlotTable {
    /// The slots a reader publishes: 0 and every occupied one.
    pub fn members(&self) -> Vec<usize> {
        std::iter::once(0)
            .chain(
                self.slots
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, held)| held.as_ref().map(|_| slot)),
            )
            .collect()
    }

    /// One past the highest occupied slot, at least 1 for slot 0.
    pub fn bound(&self) -> usize {
        self.members().last().map_or(1, |slot| slot + 1)
    }

    /// The label of `slot`, if occupied.
    #[cfg(test)]
    pub fn label(&self, slot: usize) -> Option<&str> {
        self.label_of(slot)
    }

    /// Whether the table maps `dev` under `name` already.
    pub fn knows(&self, dev: u32, name: &str) -> bool {
        self.devs.get(&dev).is_some_and(|a| a.name == name)
    }

    /// The disks beyond the cap, mapped to slot 0.
    pub fn overflow(&self) -> usize {
        self.devs.values().filter(|a| a.slot == 0).count()
    }

    /// Record that BPF reported a request completing on `dev`, named `name`.
    #[cfg(test)]
    pub fn observe(&mut self, dev: u32, name: &str) -> Vec<Action> {
        self.observe_or_evict(dev, name, || None)
    }

    /// Record that BPF reported a request completing on `dev`, named `name`;
    /// when every slot is taken, release
    /// the slot `evict` names (the reader's choice: one whose disk has been
    /// idle long enough) and give it to this disk.
    pub fn observe_or_evict(
        &mut self,
        dev: u32,
        name: &str,
        evict: impl FnOnce() -> Option<usize>,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.knows(dev, name) {
            return actions;
        }
        // The same dev_t under another name is another disk.
        if self.devs.contains_key(&dev) {
            self.unmap(dev, &mut actions);
        }
        let label = canonical(name);
        let held = self
            .slots
            .iter()
            .position(|held| held.as_deref() == Some(label.as_str()));
        let slot = match held {
            Some(slot) => slot,
            None => {
                let free = self.slots.iter().skip(1).position(Option::is_none);
                let slot = match free {
                    Some(free) => Some(free + 1),
                    None => match evict() {
                        Some(slot) if slot != 0 && self.label_of(slot).is_some() => {
                            self.release(slot, &mut actions);
                            Some(slot)
                        }
                        _ => None,
                    },
                };
                match slot {
                    Some(slot) => {
                        self.slots[slot] = Some(label.clone());
                        actions.push(Action::Assign { slot, label });
                        slot
                    }
                    None => 0,
                }
            }
        };
        self.devs.insert(
            dev,
            Alias {
                slot,
                name: name.to_string(),
            },
        );
        actions.push(Action::Map { dev, slot });
        actions
    }

    fn label_of(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot)?.as_deref()
    }

    /// Drop every mapping whose disk is not in `present` (name to its sysfs
    /// `dev`, `None` for a disk with no `dev`, such as a hidden NVMe path), or
    /// whose sysfs `dev` is no longer the one it was mapped by; release a slot
    /// left with no disk.
    pub fn retain(&mut self, present: &HashMap<String, Option<u32>>) -> Vec<Action> {
        let mut gone: Vec<u32> = self
            .devs
            .iter()
            .filter(|(dev, alias)| match present.get(&alias.name) {
                None => true,
                Some(Some(sysfs_dev)) => sysfs_dev != *dev,
                Some(None) => false,
            })
            .map(|(dev, _)| *dev)
            .collect();
        gone.sort_unstable();
        let mut actions = Vec::new();
        for dev in gone {
            self.unmap(dev, &mut actions);
        }
        actions
    }

    fn unmap(&mut self, dev: u32, actions: &mut Vec<Action>) {
        let Some(alias) = self.devs.remove(&dev) else {
            return;
        };
        actions.push(Action::Unmap { dev });
        if alias.slot != 0 && !self.devs.values().any(|a| a.slot == alias.slot) {
            self.slots[alias.slot] = None;
            actions.push(Action::Release { slot: alias.slot });
            self.unpark(actions);
        }
    }

    /// Release `slot` and unmap every dev_t mapped to it.
    fn release(&mut self, slot: usize, actions: &mut Vec<Action>) {
        let mut devs: Vec<u32> = self
            .devs
            .iter()
            .filter(|(_, a)| a.slot == slot)
            .map(|(d, _)| *d)
            .collect();
        devs.sort_unstable();
        for dev in devs {
            self.devs.remove(&dev);
            actions.push(Action::Unmap { dev });
        }
        self.slots[slot] = None;
        actions.push(Action::Release { slot });
    }

    /// Release `slot` (the reader's idle choice) and unmap every disk parked
    /// in slot 0, so a parked disk that is active takes it.
    pub fn reclaim(&mut self, slot: usize) -> Vec<Action> {
        let mut actions = Vec::new();
        if slot != 0 && self.slots.get(slot).is_some_and(Option::is_some) {
            self.release(slot, &mut actions);
            self.unpark(&mut actions);
        }
        actions
    }

    /// Unmap every disk parked in slot 0, so the next of them to complete a
    /// request takes a slot that has come free. Choosing some would risk
    /// choosing idle ones and leaving the slot empty while active disks stay
    /// in slot 0; the rest report again and park, a few lookup updates on a
    /// host with more disks than slots.
    fn unpark(&mut self, actions: &mut Vec<Action>) {
        let mut parked: Vec<u32> = self
            .devs
            .iter()
            .filter(|(_, a)| a.slot == 0)
            .map(|(d, _)| *d)
            .collect();
        parked.sort_unstable();
        for dev in parked {
            self.devs.remove(&dev);
            actions.push(Action::Unmap { dev });
        }
    }
}

/// The label for a disk name: a hidden NVMe path `nvme<s>c<c>n<n>` is
/// labelled as its head disk `nvme<s>n<n>`; any other name is itself.
pub fn canonical(name: &str) -> String {
    let parse = || -> Option<String> {
        let rest = name.strip_prefix("nvme")?;
        let (subsystem, rest) = rest.split_once('c')?;
        let (controller, namespace) = rest.split_once('n')?;
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        (digits(subsystem) && digits(controller) && digits(namespace))
            .then(|| format!("nvme{subsystem}n{namespace}"))
    };
    parse().unwrap_or_else(|| name.to_string())
}

/// Every entry of `sys_block`, by the kernel's disk name, with its `dev` as
/// the kernel's `dev_t` (`None` when it has no readable `dev`). sysfs writes a
/// `/` in a disk name as `!` (AoE's `etherd/e1.1` is `etherd!e1.1`).
pub fn scan(sys_block: &Path) -> std::io::Result<HashMap<String, Option<u32>>> {
    let mut present = HashMap::new();
    for entry in std::fs::read_dir(sys_block)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(|n| n.replace('!', "/")) else {
            continue;
        };
        let dev = std::fs::read_to_string(entry.path().join("dev"))
            .ok()
            .and_then(|s| super::filesystems::kernel_dev(s.trim()));
        present.insert(name, dev);
    }
    Ok(present)
}

/// The labels slot `slot` publishes.
pub fn labels(label: &str) -> BTreeMap<String, String> {
    [("device".to_string(), label.to_string())].into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reported_disk_takes_the_lowest_free_slot() {
        let mut t = SlotTable::default();
        assert_eq!(
            t.observe(259 << 20, "nvme0n1"),
            vec![
                Action::Assign {
                    slot: 1,
                    label: "nvme0n1".into()
                },
                Action::Map {
                    dev: 259 << 20,
                    slot: 1
                },
            ]
        );
        assert_eq!(
            t.observe(8 << 20, "sda"),
            vec![
                Action::Assign {
                    slot: 2,
                    label: "sda".into()
                },
                Action::Map {
                    dev: 8 << 20,
                    slot: 2
                },
            ]
        );
        // Reported again: nothing to do.
        assert!(t.observe(259 << 20, "nvme0n1").is_empty());
        assert_eq!(t.members(), vec![0, 1, 2]);
        assert_eq!(t.bound(), 3);
    }

    #[test]
    fn hidden_nvme_paths_share_their_head_disks_slot() {
        assert_eq!(canonical("nvme0c0n1"), "nvme0n1");
        assert_eq!(canonical("nvme12c3n45"), "nvme12n45");
        assert_eq!(canonical("nvme0n1"), "nvme0n1");
        assert_eq!(canonical("nvme0n1p2"), "nvme0n1p2");
        assert_eq!(canonical("sda"), "sda");
        let mut t = SlotTable::default();
        t.observe(1, "nvme0c0n1");
        assert_eq!(
            t.observe(2, "nvme0c1n1"),
            vec![Action::Map { dev: 2, slot: 1 }]
        );
        assert_eq!(t.label(1), Some("nvme0n1"));
        // One path leaves: the slot stays for the other.
        let present: HashMap<String, Option<u32>> = [("nvme0c1n1".to_string(), None)].into();
        assert_eq!(t.retain(&present), vec![Action::Unmap { dev: 1 }]);
        // The last one leaves: the slot is released.
        assert_eq!(
            t.retain(&HashMap::new()),
            vec![Action::Unmap { dev: 2 }, Action::Release { slot: 1 }]
        );
    }

    #[test]
    fn a_dev_t_under_a_new_name_is_a_new_disk() {
        let mut t = SlotTable::default();
        t.observe(7, "nvme1n1");
        t.observe(8, "sda");
        assert_eq!(
            t.observe(7, "nvme2n1"),
            vec![
                Action::Unmap { dev: 7 },
                Action::Release { slot: 1 },
                Action::Assign {
                    slot: 1,
                    label: "nvme2n1".into()
                },
                Action::Map { dev: 7, slot: 1 },
            ]
        );
    }

    #[test]
    fn a_disk_whose_sysfs_dev_changed_is_unmapped() {
        let mut t = SlotTable::default();
        t.observe(7, "sdb");
        let present: HashMap<String, Option<u32>> = [("sdb".to_string(), Some(9))].into();
        assert_eq!(
            t.retain(&present),
            vec![Action::Unmap { dev: 7 }, Action::Release { slot: 1 }]
        );
        assert_eq!(t.members(), vec![0]);
    }

    #[test]
    fn disks_beyond_the_cap_map_to_other() {
        let mut t = SlotTable::default();
        for i in 0..(MAX_BLOCK_DEVICES as u32 - 1) {
            t.observe(100 + i, &format!("sd{i}"));
        }
        assert_eq!(t.bound(), MAX_BLOCK_DEVICES);
        assert_eq!(
            t.observe(999, "sdzz"),
            vec![Action::Map { dev: 999, slot: 0 }]
        );
        assert_eq!(t.overflow(), 1);
        // Leaving does not release slot 0.
        let present: HashMap<String, Option<u32>> = (0..(MAX_BLOCK_DEVICES as u32 - 1))
            .map(|i| (format!("sd{i}"), Some(100 + i)))
            .collect();
        assert_eq!(t.retain(&present), vec![Action::Unmap { dev: 999 }]);
    }

    #[test]
    fn a_freed_slot_unparks_the_disks_in_other() {
        let mut t = SlotTable::default();
        for i in 0..(MAX_BLOCK_DEVICES as u32 - 1) {
            t.observe(100 + i, &format!("sd{i}"));
        }
        t.observe(999, "late");
        assert_eq!(t.overflow(), 1);
        let present: HashMap<String, Option<u32>> = (1..(MAX_BLOCK_DEVICES as u32 - 1))
            .map(|i| (format!("sd{i}"), Some(100 + i)))
            .chain([("late".to_string(), Some(999))])
            .collect();
        // sd0 leaves: its slot is released and "late" is unmapped to report again.
        assert_eq!(
            t.retain(&present),
            vec![
                Action::Unmap { dev: 100 },
                Action::Release { slot: 1 },
                Action::Unmap { dev: 999 },
            ]
        );
        assert_eq!(t.overflow(), 0);
        // Its next report takes the freed slot.
        assert_eq!(
            t.observe(999, "late"),
            vec![
                Action::Assign {
                    slot: 1,
                    label: "late".into()
                },
                Action::Map { dev: 999, slot: 1 },
            ]
        );
    }

    #[test]
    fn a_freed_slot_unparks_every_parked_disk() {
        let mut t = SlotTable::default();
        for i in 0..(MAX_BLOCK_DEVICES as u32 - 1) {
            t.observe(100 + i, &format!("sd{i}"));
        }
        t.observe(998, "late1");
        t.observe(999, "late2");
        assert_eq!(t.overflow(), 2);
        let present: HashMap<String, Option<u32>> = (1..(MAX_BLOCK_DEVICES as u32 - 1))
            .map(|i| (format!("sd{i}"), Some(100 + i)))
            .chain([
                ("late1".to_string(), Some(998)),
                ("late2".to_string(), Some(999)),
            ])
            .collect();
        assert_eq!(
            t.retain(&present),
            vec![
                Action::Unmap { dev: 100 },
                Action::Release { slot: 1 },
                Action::Unmap { dev: 998 },
                Action::Unmap { dev: 999 },
            ]
        );
        assert_eq!(t.overflow(), 0);
        // The first to report takes the slot; the other parks again.
        t.observe(999, "late2");
        assert_eq!(
            t.observe(998, "late1"),
            vec![Action::Map { dev: 998, slot: 0 }]
        );
    }

    #[test]
    fn reclaiming_an_idle_slot_unparks_the_disks_in_other() {
        let mut t = SlotTable::default();
        for i in 0..(MAX_BLOCK_DEVICES as u32 - 1) {
            t.observe(100 + i, &format!("sd{i}"));
        }
        t.observe(999, "late");
        assert_eq!(t.overflow(), 1);
        assert_eq!(
            t.reclaim(3),
            vec![
                Action::Unmap { dev: 102 },
                Action::Release { slot: 3 },
                Action::Unmap { dev: 999 },
            ]
        );
        assert_eq!(t.overflow(), 0);
        assert_eq!(
            t.observe(999, "late"),
            vec![
                Action::Assign {
                    slot: 3,
                    label: "late".into()
                },
                Action::Map { dev: 999, slot: 3 },
            ]
        );
        // Slot 0 and an empty slot are not reclaimed.
        assert!(t.reclaim(0).is_empty());
        t.reclaim(3);
        assert!(t.reclaim(3).is_empty());
    }

    #[test]
    fn a_full_table_evicts_the_slot_the_reader_names() {
        let mut t = SlotTable::default();
        for i in 0..(MAX_BLOCK_DEVICES as u32 - 1) {
            t.observe(100 + i, &format!("loop{i}"));
        }
        // No candidate: the new disk goes to other.
        assert_eq!(
            t.observe_or_evict(7, "sdb", || None),
            vec![Action::Map { dev: 7, slot: 0 }]
        );
        // A candidate: that slot is released and given to the new disk.
        assert_eq!(
            t.observe_or_evict(8, "sdc", || Some(5)),
            vec![
                Action::Unmap { dev: 104 },
                Action::Release { slot: 5 },
                Action::Assign {
                    slot: 5,
                    label: "sdc".into()
                },
                Action::Map { dev: 8, slot: 5 },
            ]
        );
        assert_eq!(t.label(5), Some("sdc"));
    }

    #[test]
    fn scan_reads_names_and_devs() {
        let dir = tempfile::tempdir().unwrap();
        let make = |name: &str, dev: Option<&str>| {
            let d = dir.path().join(name);
            std::fs::create_dir(&d).unwrap();
            if let Some(dev) = dev {
                std::fs::write(d.join("dev"), format!("{dev}\n")).unwrap();
            }
        };
        make("nvme0n1", Some("259:0"));
        make("nvme0c0n1", None);
        make("etherd!e1.1", Some("152:16"));
        let got = scan(dir.path()).unwrap();
        assert_eq!(got.get("nvme0n1"), Some(&Some(259 << 20)));
        assert_eq!(got.get("nvme0c0n1"), Some(&None));
        assert_eq!(got.get("etherd/e1.1"), Some(&Some((152 << 20) | 16)));
        assert_eq!(got.len(), 3);
    }
}
