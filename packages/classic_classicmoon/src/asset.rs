use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::querier::{query_balance, query_token_balance};
use classic_bindings::{TerraMsg, TerraQuerier, TerraQuery};
use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, Decimal, MessageInfo,
    QuerierWrapper, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.info)
    }
}

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

impl Asset {
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    pub fn compute_tax(&self, querier: &QuerierWrapper<TerraQuery>) -> StdResult<Uint128> {
        let amount = self.amount;
        if let AssetInfo::NativeToken { denom } = &self.info {
            let terra_querier = TerraQuerier::new(querier);
            let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
            let tax_cap: Uint128 = (terra_querier.query_tax_cap(denom.to_string())?).cap;
            Ok(std::cmp::min(
                amount.checked_sub(amount.multiply_ratio(
                    DECIMAL_FRACTION,
                    DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                ))?,
                tax_cap,
            ))
        } else {
            Ok(Uint128::zero())
        }
    }

    pub fn deduct_tax(&self, querier: &QuerierWrapper<TerraQuery>) -> StdResult<Coin> {
        let amount = self.amount;
        if let AssetInfo::NativeToken { denom } = &self.info {
            Ok(Coin {
                denom: denom.to_string(),
                amount: amount.checked_sub(self.compute_tax(querier)?)?,
            })
        } else {
            Err(StdError::generic_err("cannot deduct tax from token asset"))
        }
    }

    pub fn into_msg(
        self,
        querier: &QuerierWrapper<TerraQuery>,
        recipient: Addr,
    ) -> StdResult<CosmosMsg<TerraMsg>> {
        let amount = self.amount;

        match &self.info {
            AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
                    amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::NativeToken { .. } => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![self.deduct_tax(querier)?],
            })),
        }
    }

    pub fn into_submsg(
        self,
        querier: &QuerierWrapper<TerraQuery>,
        recipient: Addr,
    ) -> StdResult<SubMsg<TerraMsg>> {
        Ok(SubMsg::new(self.into_msg(querier, recipient)?))
    }

    pub fn assert_sent_native_token_balance(&self, message_info: &MessageInfo) -> StdResult<()> {
        if let AssetInfo::NativeToken { denom } = &self.info {
            match message_info.funds.iter().find(|x| x.denom == *denom) {
                Some(coin) => {
                    if self.amount == coin.amount {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
                None => {
                    if self.amount.is_zero() {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn to_raw(&self, api: &dyn Api) -> StdResult<AssetRaw> {
        Ok(AssetRaw {
            info: match &self.info {
                AssetInfo::NativeToken { denom } => AssetInfoRaw::NativeToken {
                    denom: denom.to_string(),
                },
                AssetInfo::Token { contract_addr } => AssetInfoRaw::Token {
                    contract_addr: api.addr_canonicalize(contract_addr.as_str())?,
                },
            },
            amount: self.amount,
        })
    }
}

/// AssetInfo contract_addr is usually passed from the cw20 hook
/// so we can trust the contract_addr is properly validated.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token { contract_addr: String },
    NativeToken { denom: String },
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetInfo::NativeToken { denom } => write!(f, "{}", denom),
            AssetInfo::Token { contract_addr } => write!(f, "{}", contract_addr),
        }
    }
}

impl AssetInfo {
    pub fn to_raw(&self, api: &dyn Api) -> StdResult<AssetInfoRaw> {
        match self {
            AssetInfo::NativeToken { denom } => Ok(AssetInfoRaw::NativeToken {
                denom: denom.to_string(),
            }),
            AssetInfo::Token { contract_addr } => Ok(AssetInfoRaw::Token {
                contract_addr: api.addr_canonicalize(contract_addr.as_str())?,
            }),
        }
    }

    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::NativeToken { .. } => true,
            AssetInfo::Token { .. } => false,
        }
    }
    pub fn query_pool(
        &self,
        querier: &QuerierWrapper<TerraQuery>,
        api: &dyn Api,
        pool_addr: Addr,
    ) -> StdResult<Uint128> {
        match self {
            AssetInfo::Token { contract_addr, .. } => query_token_balance(
                querier,
                api.addr_validate(&contract_addr).unwrap(),
                pool_addr,
            ),
            AssetInfo::NativeToken { denom, .. } => {
                query_balance(querier, pool_addr, denom.to_string())
            }
        }
    }

    pub fn equal(&self, asset: &AssetInfo) -> bool {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfo::Token { contract_addr, .. } => self_contract_addr == contract_addr,
                    AssetInfo::NativeToken { .. } => false,
                }
            }
            AssetInfo::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfo::Token { .. } => false,
                    AssetInfo::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AssetRaw {
    pub info: AssetInfoRaw,
    pub amount: Uint128,
}

impl AssetRaw {
    pub fn to_normal(&self, api: &dyn Api) -> StdResult<Asset> {
        Ok(Asset {
            info: match &self.info {
                AssetInfoRaw::NativeToken { denom } => AssetInfo::NativeToken {
                    denom: denom.to_string(),
                },
                AssetInfoRaw::Token { contract_addr } => AssetInfo::Token {
                    contract_addr: api.addr_humanize(contract_addr)?.to_string(),
                },
            },
            amount: self.amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum AssetInfoRaw {
    Token { contract_addr: CanonicalAddr },
    NativeToken { denom: String },
}

impl AssetInfoRaw {
    pub fn to_normal(&self, api: &dyn Api) -> StdResult<AssetInfo> {
        match self {
            AssetInfoRaw::NativeToken { denom } => Ok(AssetInfo::NativeToken {
                denom: denom.to_string(),
            }),
            AssetInfoRaw::Token { contract_addr } => Ok(AssetInfo::Token {
                contract_addr: api.addr_humanize(contract_addr)?.to_string(),
            }),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AssetInfoRaw::NativeToken { denom } => denom.as_bytes(),
            AssetInfoRaw::Token { contract_addr } => contract_addr.as_slice(),
        }
    }

    pub fn equal(&self, asset: &AssetInfoRaw) -> bool {
        match self {
            AssetInfoRaw::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfoRaw::Token { contract_addr, .. } => {
                        self_contract_addr == contract_addr
                    }
                    AssetInfoRaw::NativeToken { .. } => false,
                }
            }
            AssetInfoRaw::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfoRaw::Token { .. } => false,
                    AssetInfoRaw::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ClassicmoonInfo {
    pub asset_infos: [AssetInfo; 2],
    pub contract_addr: String,
    pub liquidity_k_value: Uint128,
    pub vesting_epoch: u64,
    pub autoburn_epoch: u64,
    pub asset_decimals: [u8; 2],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ClassicmoonInfoRaw {
    pub asset_infos: [AssetInfoRaw; 2],
    pub contract_addr: CanonicalAddr,
    pub liquidity_k_value: Uint128,
    pub vesting_epoch: u64,
    pub autoburn_epoch: u64,
    pub asset_decimals: [u8; 2],
}

impl ClassicmoonInfoRaw {
    pub fn to_normal(&self, api: &dyn Api) -> StdResult<ClassicmoonInfo> {
        Ok(ClassicmoonInfo {
            liquidity_k_value: self.liquidity_k_value,
            contract_addr: api.addr_humanize(&self.contract_addr)?.to_string(),
            vesting_epoch: self.vesting_epoch,
            autoburn_epoch: self.autoburn_epoch,
            asset_infos: [
                self.asset_infos[0].to_normal(api)?,
                self.asset_infos[1].to_normal(api)?,
            ],
            asset_decimals: self.asset_decimals,
        })
    }

    pub fn query_pools(
        &self,
        querier: &QuerierWrapper<TerraQuery>,
        api: &dyn Api,
        contract_addr: Addr,
    ) -> StdResult<[Asset; 2]> {
        let info_0: AssetInfo = self.asset_infos[0].to_normal(api)?;
        let info_1: AssetInfo = self.asset_infos[1].to_normal(api)?;
        Ok([
            Asset {
                amount: info_0.query_pool(querier, api, contract_addr.clone())?,
                info: info_0,
            },
            Asset {
                amount: info_1.query_pool(querier, api, contract_addr)?,
                info: info_1,
            },
        ])
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DynamicInfo {
    pub total_lunc_burn_amount: Uint128,
    pub total_ustc_burn_amount: Uint128,
    pub total_minted_clsm_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DynamicInfoRaw {
    pub total_lunc_burn_amount: Uint128,
    pub total_ustc_burn_amount: Uint128,
    pub total_minted_clsm_amount: Uint128,
}

impl DynamicInfoRaw {
    pub fn to_normal(&self) -> StdResult<DynamicInfo> {
        Ok(DynamicInfo {
            total_lunc_burn_amount: self.total_lunc_burn_amount,
            total_ustc_burn_amount: self.total_ustc_burn_amount,
            total_minted_clsm_amount: self.total_minted_clsm_amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AirdropGlobal {
    pub total_dropped_amounts: Uint128,
    pub last_drop_user: String,
    pub last_drop_timestamp: u64,
    pub last_drop_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AirdropGlobalRaw {
    pub total_dropped_amounts: Uint128,
    pub last_drop_user: CanonicalAddr,
    pub last_drop_timestamp: u64,
    pub last_drop_amount: Uint128,
}

impl AirdropGlobalRaw {
    pub fn to_normal(&self, api: &dyn Api) -> StdResult<AirdropGlobal> {
        Ok(AirdropGlobal {
            total_dropped_amounts: self.total_dropped_amounts,
            last_drop_user: api.addr_humanize(&self.last_drop_user)?.to_string(),
            last_drop_timestamp: self.last_drop_timestamp,
            last_drop_amount: self.last_drop_amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, Default)]
pub struct AirdropNftInfo {
    pub dropped_amount: Uint128,
    pub last_drop_amount: Uint128,
    pub last_drop_time: u64,
}

impl AirdropNftInfo {
    pub fn to_normal(&self) -> StdResult<AirdropNftInfo> {
        Ok(AirdropNftInfo {
            dropped_amount: self.dropped_amount,
            last_drop_amount: self.last_drop_amount,
            last_drop_time: self.last_drop_time,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, Default)]
pub struct AirdropUserInfoResponse {
    pub dropped_amount: Uint128,
    pub last_drop_amount: Uint128,
    pub last_drop_time: u64,
    pub next_drop_time: u64,
    pub pending_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, Default)]
pub struct AirdropUserInfo {
    pub dropped_amount: Uint128,
    pub last_drop_amount: Uint128,
    pub last_drop_time: u64,
}

impl AirdropUserInfo {
    pub fn to_normal(&self) -> StdResult<AirdropUserInfo> {
        Ok(AirdropUserInfo {
            dropped_amount: self.dropped_amount,
            last_drop_amount: self.last_drop_amount,
            last_drop_time: self.last_drop_time,
        })
    }
}
