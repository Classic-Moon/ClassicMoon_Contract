use classic_classicmoon::asset::{AirdropGlobalRaw, AirdropNftInfo, AirdropUserInfo};
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const AIRDROP_GLOBAL: Item<AirdropGlobalRaw> = Item::new("airdrop_config");
pub const AIRDROP_NFT_INFO: Map<String, AirdropNftInfo> = Map::new("airdrop_nft_info");
pub const AIRDROP_USER_INFO: Map<Addr, AirdropUserInfo> = Map::new("airdrop_user_info");
