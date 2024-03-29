use std::cmp::{Eq, PartialEq};
use std::iter::Iterator;
use strum_macros::FromRepr;

/// Light...
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub(crate) mode: LightMode,
    pub(crate) power: bool,
    pub(crate) connected: bool,
    pub(crate) _changes_: u8,
}

#[derive(Clone, Debug, Eq, FromRepr, PartialEq)]
#[repr(u8)]
pub enum Changed {
    Mode = 1 << 0,
    Power = 1 << 1,
    // This should perhaps split into GuiChanged and DeviceChanged.
    // For now...
    Connected = 1 << 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChangeIterator {
    mask: u8,
    shift: u32,
}

impl Iterator for ChangeIterator {
    type Item = Changed;
    fn next(&mut self) -> Option<Self::Item> {
        // This can doubtlessly be improved
        let tz = self.mask.trailing_zeros();
        let discr = ((self.mask.wrapping_shr(tz)) & 1).wrapping_shl(self.shift + tz);
        self.mask = self.mask.wrapping_shr(tz + 1);
        self.shift += tz + 1;
        Self::Item::from_repr(discr)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // FIXME This assumes mask bits only contain enum repr items..
        (
            self.mask.count_ones() as usize,
            Some(self.mask.count_ones() as usize),
        )
    }
}

impl ChangeIterator {
    fn new(mask: u8) -> ChangeIterator {
        ChangeIterator { mask, shift: 0 }
    }
}

#[test]
fn change_iter() {
    assert_eq!(ChangeIterator::new(0).collect::<Vec<Changed>>(), vec![]);
    assert_eq!(
        ChangeIterator::new(1).collect::<Vec<Changed>>(),
        vec![Changed::Mode]
    );
    assert_eq!(
        ChangeIterator::new(1 << 1).collect::<Vec<Changed>>(),
        vec![Changed::Power]
    );
    assert_eq!(
        ChangeIterator::new(0x3).collect::<Vec<Changed>>(),
        vec![Changed::Mode, Changed::Power]
    );
    // below we would hit the "buggy" size_hint
    // callers need to be prepared for this according to the size_hint docs)
    //
    // which would 8 and 6 items respectively.. rather than 2 and 0..
    //
    // We never should never actually run into it in this ad-hoc implementation, but for a generic
    // one...
    //
    // A generic impl though seems blocked by other things though, like `inherent associated types`
    assert_eq!(
        ChangeIterator::new(std::u8::MAX).collect::<Vec<Changed>>(),
        vec![Changed::Mode, Changed::Power, Changed::Connected]
    );
    assert_eq!(
        ChangeIterator::new(std::u8::MAX ^ 0x7).collect::<Vec<Changed>>(),
        vec![]
    );
}

impl Default for Light {
    fn default() -> Light {
        Light {
            mode: LightMode::CCT(CCTMode {
                temp: 32.0,
                brightness: 0.0,
            }),
            power: false,
            connected: false,
            _changes_: 0,
        }
    }
}

impl Light {
    pub fn clear_changes(&mut self) {
        self._changes_ = 0;
    }
    pub fn has_changes(&self) -> bool {
        self._changes_ != 0
    }
    pub fn changes(&self) -> impl Iterator<Item = Changed> {
        ChangeIterator::new(self._changes_)
    }

    pub fn toggle_power(&mut self) {
        self.power = !self.power;
        self._changes_ |= Changed::Power as u8;
    }

    pub fn sync(&mut self, _other: &Self) {
        // FIXME..
    }
}

/// LightMode...
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum LightMode {
    CCT(CCTMode),
    HSI(HSIMode),
    Anim(AnimMode),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CCTMode {
    pub temp: f64,
    pub brightness: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HSIMode {
    pub hue: f64,
    pub saturation: f64,
    pub intensity: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimMode {
    pub scene: f64,
    pub brightness: f64,
}
