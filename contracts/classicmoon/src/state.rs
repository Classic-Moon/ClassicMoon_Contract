use classic_classicmoon::asset::ClassicmoonInfoRaw;
use cw_storage_plus::Item;

pub const CLASSICMOON_INFO: Item<ClassicmoonInfoRaw> = Item::new("classicmoon_info");
