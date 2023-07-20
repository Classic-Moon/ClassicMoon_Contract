use classic_classicmoon::asset::DynamicInfoRaw;
use cw_storage_plus::Item;

pub const DYNAMIC_INFO: Item<DynamicInfoRaw> = Item::new("dynamic_info");
